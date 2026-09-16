use super::{Tick, models::Bar1m};
use std::collections::HashMap;

type Instrument = (i32, u8);

#[derive(Debug)]
struct WorkingBar {
    bar: Bar1m,
    volume_value: f64,
}

#[derive(Debug, Default)]
struct InstrumentState {
    previous_volume: Option<i64>,
    current: Option<WorkingBar>,
}

#[derive(Debug, Default)]
pub struct BarAggregator {
    instruments: HashMap<Instrument, InstrumentState>,
}

impl BarAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one tick and returns a completed candle when the tick starts a new minute.
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

fn finish(mut working: WorkingBar) -> Bar1m {
    working.bar.avg_trade_size = if working.bar.trade_count == 0 {
        0.0
    } else {
        working.bar.volume as f32 / working.bar.trade_count as f32
    };
    working.bar
}

fn minute_start(ltt: i64) -> i64 {
    ltt.div_euclid(60) * 60
}

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

fn imbalance(buy: i64, sell: i64) -> f32 {
    let total = buy + sell;
    if total == 0 {
        0.0
    } else {
        (buy - sell) as f32 / total as f32
    }
}
