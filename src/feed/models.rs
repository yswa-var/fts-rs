use serde::Serialize;

#[derive(Debug)]
pub struct TickerPacket {
    pub response_code: u8,
    pub message_length: u16,
    pub exchange_segment: u8,
    pub security_id: i32,
    pub ltp: f32,
    pub last_trade_time: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Instrument {
    #[serde(rename = "ExchangeSegment")]
    pub exchange_segment: String,

    #[serde(rename = "SecurityId")]
    pub security_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    #[serde(rename = "RequestCode")]
    pub request_code: u8,

    #[serde(rename = "InstrumentCount")]
    pub instrument_count: usize,

    #[serde(rename = "InstrumentList")]
    pub instrument_list: Vec<Instrument>,
}
#[derive(Debug)]
pub struct DepthLevel {
    pub bid_qty: i32,
    pub ask_qty: i32,
    pub bid_orders: i16,
    pub ask_orders: i16,
    pub bid_price: f32,
    pub ask_price: f32,
}

#[derive(Debug)]
pub struct FullPacket {
    pub response_code: u8,
    pub message_length: u16,
    pub exchange_segment: u8,
    pub security_id: i32,

    pub ltp: f32,
    pub last_trade_qty: i16,
    pub last_trade_time: i32,
    pub avg_trade_price: f32,
    pub volume: i32,

    pub total_sell_qty: i32,
    pub total_buy_qty: i32,

    pub open_interest: i32,
    pub highest_oi: i32,
    pub lowest_oi: i32,

    pub day_open: f32,
    pub day_close: f32,
    pub day_high: f32,
    pub day_low: f32,

    pub depth: Vec<DepthLevel>,
}
