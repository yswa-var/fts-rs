//! One-minute bar construction and persistence for live Dhan ticks.
//!
//! The aggregator converts Redis stream entries into typed ticks, groups each
//! instrument's ticks by exchange-event minute, and sends completed bars to
//! ClickHouse through a buffered writer.

mod aggregator;
mod clickhouse;
mod models;
mod tick;

pub use aggregator::BarAggregator;
pub use clickhouse::{ClickHouseWriter, StoredBar};
use models::Bar1m;
pub use tick::Tick;
