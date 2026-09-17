use super::{models::Bar1m, Tick};
use std::collections::HashMap;

type Instrument = (i32, u8);

/// The in-progress bar plus the unrounded value needed to calculate VWAP.
#[derive(Debug)]
struct WorkingBar {
    bar: Bar1m,
    volume_value: f64,
}

/// Per-instrument state retained while live ticks are aggregated.
///
/// Cumulative volume is tracked separately so each incoming tick can be
/// converted into its incremental contribution to the current bar.
#[derive(Debug, Default)]
struct InstrumentState {
    previous_volume: Option<i64>,
    current: Option<WorkingBar>,
}

/// Builds independent one-minute OHLCV and microstructure bars per instrument.
///
/// Each instrument is keyed by its security ID and exchange segment, allowing
/// interleaved ticks from the Redis stream to be aggregated safely in one
/// consumer.
#[derive(Debug, Default)]
pub struct BarAggregator {
    instruments: HashMap<Instrument, InstrumentState>,
}

impl BarAggregator {
    /// Creates an empty aggregator with no active instrument windows.
    pub fn new() -> Self {
        Self::default()
    }

    /// Incorporates a live tick and returns the preceding bar at a minute boundary.
    ///
    /// Minute boundaries are based on Dhan's last-trade time rather than local
    /// arrival time. Ticks for an already-open minute update its OHLCV and
    /// market-depth fields; late ticks from older minutes are ignored so a
    /// finalized bar is never rewritten.
    pub fn on_tick(&mut self, tick: Tick) -> Option<Bar1m> {
        let instrument = (tick.security_id, tick.exchange_segment);
        let state = self.instruments.entry(instrument).or_default();
        let minute = minute_start(tick.last_trade_time);
        let volume_delta =
            cumulative_delta(&mut state.previous_volume, tick.volume, tick.last_trade_qty);

        let previous = match state.current.as_ref() {
            Some(current) if current.bar.ts < minute => state.current.take().map(|bar| finish(bar)),
            Some(current) if current.bar.ts == minute => None,
            Some(_) => {
                eprintln!(
                    "ignoring out-of-order tick: security_id={}, ltt={}",
                    tick.security_id, tick.last_trade_time
                );
                return None;
            }
            None => None,
        };

        if state.current.is_none() {
            state.current = Some(WorkingBar {
                bar: Bar1m {
                    security_id: tick.security_id,
                    exchange_segment: tick.exchange_segment,
                    ts: minute,
                    open: tick.ltp,
                    high: tick.ltp,
                    low: tick.ltp,
                    close: tick.ltp,
                    volume: 0,
                    trade_count: 0,
                    avg_trade_size: 0.0,
                    vwap: tick.ltp,
                    atp_close: tick.avg_trade_price,
                    buy_qty_close: tick.total_buy_qty,
                    sell_qty_close: tick.total_sell_qty,
                    buy_sell_imbalance: imbalance(tick.total_buy_qty, tick.total_sell_qty),
                    bid_close: tick.depth_0_bid_price,
                    ask_close: tick.depth_0_ask_price,
                    spread_close: tick.depth_0_ask_price - tick.depth_0_bid_price,
                    bid_depth_close: tick.depth_0_bid_qty,
                    ask_depth_close: tick.depth_0_ask_qty,
                    depth_imbalance: imbalance(tick.depth_0_bid_qty, tick.depth_0_ask_qty),
                    open_interest_open: tick.open_interest,
                    open_interest_close: tick.open_interest,
                    open_interest_change: 0,
                    tick_count: 0,
                    price_change: 0.0,
                    range: 0.0,
                    first_ltt: tick.last_trade_time,
                    last_ltt: tick.last_trade_time,
                },
                volume_value: 0.0,
            });
        }

        let current = state
            .current
            .as_mut()
            .expect("working bar was just created");
        current.bar.high = current.bar.high.max(tick.ltp);
        current.bar.low = current.bar.low.min(tick.ltp);
        current.bar.close = tick.ltp;
        current.bar.volume += volume_delta;
        current.bar.tick_count += 1;
        if volume_delta > 0 {
            current.bar.trade_count += 1;
            current.volume_value += f64::from(tick.ltp) * volume_delta as f64;
        }
        current.bar.avg_trade_size = if current.bar.trade_count == 0 {
            0.0
        } else {
            current.bar.volume as f32 / current.bar.trade_count as f32
        };
        current.bar.vwap = if current.bar.volume == 0 {
            current.bar.close
        } else {
            (current.volume_value / current.bar.volume as f64) as f32
        };
        current.bar.atp_close = tick.avg_trade_price;
        current.bar.buy_qty_close = tick.total_buy_qty;
        current.bar.sell_qty_close = tick.total_sell_qty;
        current.bar.buy_sell_imbalance = imbalance(tick.total_buy_qty, tick.total_sell_qty);
        current.bar.bid_close = tick.depth_0_bid_price;
        current.bar.ask_close = tick.depth_0_ask_price;
        current.bar.spread_close = tick.depth_0_ask_price - tick.depth_0_bid_price;
        current.bar.bid_depth_close = tick.depth_0_bid_qty;
        current.bar.ask_depth_close = tick.depth_0_ask_qty;
        current.bar.depth_imbalance = imbalance(tick.depth_0_bid_qty, tick.depth_0_ask_qty);
        current.bar.open_interest_close = tick.open_interest;
        current.bar.open_interest_change = tick.open_interest - current.bar.open_interest_open;
        current.bar.price_change = current.bar.close - current.bar.open;
        current.bar.range = current.bar.high - current.bar.low;
        current.bar.last_ltt = tick.last_trade_time;

        previous
    }
}

/// Finalizes calculated fields before releasing a completed bar.
fn finish(mut working: WorkingBar) -> Bar1m {
    working.bar.avg_trade_size = if working.bar.trade_count == 0 {
        0.0
    } else {
        working.bar.volume as f32 / working.bar.trade_count as f32
    };
    working.bar
}

/// Rounds an exchange timestamp down to its UTC minute boundary.
fn minute_start(ltt: i64) -> i64 {
    ltt.div_euclid(60) * 60
}

/// Converts a cumulative exchange volume counter into a per-tick increment.
///
/// If the counter moves backwards (for example, after an exchange reset), the
/// reported last-trade quantity is used as a safe fallback.
fn cumulative_delta(previous: &mut Option<i64>, value: i64, fallback: i64) -> i64 {
    let delta = previous.map_or(fallback.max(0), |old| {
        if value >= old {
            value - old
        } else {
            fallback.max(0)
        }
    });
    *previous = Some(value);
    delta
}

/// Calculates normalized buy-versus-sell pressure, returning zero when empty.
fn imbalance(buy: i64, sell: i64) -> f32 {
    let total = buy + sell;
    if total == 0 {
        0.0
    } else {
        (buy - sell) as f32 / total as f32
    }
}
