/// A completed one-minute bar derived from exchange-event-time live ticks.
///
/// Alongside OHLCV, each bar preserves end-of-minute order-flow, top-of-book,
/// depth, and open-interest signals for downstream analytics.
#[derive(Debug, Clone)]
pub struct Bar1m {
    pub security_id: i32,
    pub exchange_segment: u8,
    pub ts: i64, // minute start from LTT
    // bar_ts comes from market event time, never local receive time

    // Price
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,

    // Activity
    pub volume: i64,
    pub trade_count: u32,
    pub avg_trade_size: f32,

    // VWAP / ATP
    pub vwap: f32,
    pub atp_close: f32,

    // Order flow
    pub buy_qty_close: i64,
    pub sell_qty_close: i64,
    pub buy_sell_imbalance: f32,

    // Best bid / ask
    pub bid_close: f32,
    pub ask_close: f32,
    pub spread_close: f32,

    // Depth
    pub bid_depth_close: i64,
    pub ask_depth_close: i64,
    pub depth_imbalance: f32,

    // OI
    pub open_interest_open: i64,
    pub open_interest_close: i64,
    pub open_interest_change: i64,

    // Useful volatility/activity stats
    pub tick_count: u32,
    pub price_change: f32,
    pub range: f32,

    // Source quality
    pub first_ltt: i64,
    pub last_ltt: i64,
}
