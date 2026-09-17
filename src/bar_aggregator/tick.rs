use redis::{from_redis_value, RedisResult, Value};
use std::collections::HashMap;

/// The subset of a Redis `ticks` stream entry needed to construct a live bar.
///
/// Values are decoded from the string-backed Redis fields produced by the feed
/// publisher, with unavailable top-of-book fields defaulting to zero.
#[derive(Debug, Clone)]
pub struct Tick {
    pub security_id: i32,
    pub exchange_segment: u8,
    pub ltp: f32,
    pub last_trade_qty: i64,
    pub last_trade_time: i64,
    pub avg_trade_price: f32,
    pub volume: i64,
    pub total_sell_qty: i64,
    pub total_buy_qty: i64,
    pub open_interest: i64,
    pub depth_0_bid_qty: i64,
    pub depth_0_ask_qty: i64,
    pub depth_0_bid_price: f32,
    pub depth_0_ask_price: f32,
}

impl Tick {
    /// Decodes one Redis stream entry into the normalized tick used by the aggregator.
    ///
    /// Core trade and instrument fields are required; malformed or missing
    /// values fail conversion, while optional level-one depth values default.
    pub fn from_stream_entry(fields: &HashMap<String, Value>) -> RedisResult<Self> {
        Ok(Self {
            security_id: required(fields, "security_id")?,
            exchange_segment: required(fields, "exchange_segment")?,
            ltp: required(fields, "ltp")?,
            last_trade_qty: required(fields, "last_trade_qty")?,
            last_trade_time: required(fields, "last_trade_time")?,
            avg_trade_price: required(fields, "avg_trade_price")?,
            volume: required(fields, "volume")?,
            total_sell_qty: required(fields, "total_sell_qty")?,
            total_buy_qty: required(fields, "total_buy_qty")?,
            open_interest: required(fields, "open_interest")?,
            depth_0_bid_qty: optional(fields, "depth_0_bid_qty"),
            depth_0_ask_qty: optional(fields, "depth_0_ask_qty"),
            depth_0_bid_price: optional(fields, "depth_0_bid_price"),
            depth_0_ask_price: optional(fields, "depth_0_ask_price"),
        })
    }
}

/// Reads a required, typed field and reports a Redis-style conversion error.
fn required<T: redis::FromRedisValue>(
    fields: &HashMap<String, Value>,
    name: &str,
) -> RedisResult<T> {
    fields.get(name).map_or_else(
        || {
            Err(redis::RedisError::from((
                redis::ErrorKind::UnexpectedReturnType,
                "missing stream field",
                name.to_owned(),
            )))
        },
        |value| from_redis_value(value.clone()).map_err(Into::into),
    )
}

/// Reads an optional typed field, substituting its type's default on failure.
fn optional<T: redis::FromRedisValue + Default>(fields: &HashMap<String, Value>, name: &str) -> T {
    fields
        .get(name)
        .and_then(|value| from_redis_value(value.clone()).ok())
        .unwrap_or_default()
}
