//! Dhan WebSocket ingestion pipeline.
//!
//! This module connects to Dhan, subscribes to instruments, decodes binary
//! market-data packets, and appends the resulting ticks to Redis.

pub mod client;
pub mod models;
pub mod parser;
pub mod publisher;
pub mod reconnect;

pub use client::run;
pub use models::{Instrument, Subscription, TickerPacket};
pub use parser::parse_ticker;
pub use publisher::RedisPublisher;
