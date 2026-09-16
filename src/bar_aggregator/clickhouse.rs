use super::Bar1m;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;
use std::{error::Error, time::Duration};
use tokio::{sync::mpsc, time::MissedTickBehavior};

pub type ClickHouseError = Box<dyn Error + Send + Sync>;

const CREATE_BARS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS bars_1m (
    security_id Int32,
    exchange_segment UInt8,
    ts DateTime,
    open Float32,
    high Float32,
    low Float32,
    close Float32,
    volume Int64,
    vwap Nullable(Float32),
    trade_count Nullable(UInt32),
    avg_trade_size Nullable(Float32),
    buy_sell_imbalance Nullable(Float32),
    spread_close Nullable(Float32),
    depth_imbalance Nullable(Float32),
    open_interest_open Nullable(Int64),
    open_interest_close Nullable(Int64),
    open_interest_change Nullable(Int64),
    tick_count Nullable(UInt32),
    source Enum8('historical' = 1, 'live' = 2)
)
ENGINE = ReplacingMergeTree
ORDER BY (security_id, exchange_segment, ts)
"#;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BarSource {
    #[allow(dead_code)]
    Historical,
    Live,
}

/// The common ClickHouse row used by both historical and live ingestion.
#[derive(Debug, Clone, Serialize)]
pub struct StoredBar {
    pub security_id: i32,
    pub exchange_segment: u8,
    pub ts: String,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: i64,
    pub vwap: Option<f32>,
    pub trade_count: Option<u32>,
    pub avg_trade_size: Option<f32>,
    pub buy_sell_imbalance: Option<f32>,
    pub spread_close: Option<f32>,
    pub depth_imbalance: Option<f32>,
    pub open_interest_open: Option<i64>,
    pub open_interest_close: Option<i64>,
    pub open_interest_change: Option<i64>,
    pub tick_count: Option<u32>,
    pub source: BarSource,
}

impl StoredBar {
    #[allow(dead_code)]
    pub fn historical(
        security_id: i32,
        exchange_segment: u8,
        ts: i64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        volume: i64,
    ) -> Result<Self, ClickHouseError> {
        Ok(Self {
            security_id,
            exchange_segment,
            ts: format_timestamp(ts)?,
            open,
            high,
            low,
            close,
            volume,
            vwap: None,
            trade_count: None,
            avg_trade_size: None,
            buy_sell_imbalance: None,
            spread_close: None,
            depth_imbalance: None,
            open_interest_open: None,
            open_interest_close: None,
            open_interest_change: None,
            tick_count: None,
            source: BarSource::Historical,
        })
    }
}

impl TryFrom<Bar1m> for StoredBar {
    type Error = ClickHouseError;

    fn try_from(bar: Bar1m) -> Result<Self, Self::Error> {
        Ok(Self {
            security_id: bar.security_id,
            exchange_segment: bar.exchange_segment,
            ts: format_timestamp(bar.ts)?,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
            vwap: Some(bar.vwap),
            trade_count: Some(bar.trade_count),
            avg_trade_size: Some(bar.avg_trade_size),
            buy_sell_imbalance: Some(bar.buy_sell_imbalance),
            spread_close: Some(bar.spread_close),
            depth_imbalance: Some(bar.depth_imbalance),
            open_interest_open: Some(bar.open_interest_open),
            open_interest_close: Some(bar.open_interest_close),
            open_interest_change: Some(bar.open_interest_change),
            tick_count: Some(bar.tick_count),
            source: BarSource::Live,
        })
    }
}

pub struct ClickHouseWriter {
    client: ClickHouse,
    receiver: mpsc::Receiver<StoredBar>,
    batch_size: usize,
    flush_interval: Duration,
}

impl ClickHouseWriter {
    pub async fn start_from_env() -> Result<mpsc::Sender<StoredBar>, ClickHouseError> {
        let batch_size = env_usize("CLICKHOUSE_BATCH_SIZE", 1_000).clamp(500, 5_000);
        let channel_capacity = env_usize("CLICKHOUSE_CHANNEL_CAPACITY", 5_000).max(batch_size);
        let flush_interval =
            Duration::from_millis(env_u64("CLICKHOUSE_FLUSH_INTERVAL_MS", 1_000).max(1));
        let client = ClickHouse::connect().await?;
        let (sender, receiver) = mpsc::channel(channel_capacity);

        let writer = Self {
            client,
            receiver,
            batch_size,
            flush_interval,
        };
        tokio::spawn(writer.run());

        Ok(sender)
    }

    async fn run(mut self) {
        let mut batch = Vec::with_capacity(self.batch_size);
        let mut flush_timer = tokio::time::interval(self.flush_interval);
        flush_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                item = self.receiver.recv() => {
                    match item {
                        Some(bar) => {
                            batch.push(bar);
                            if batch.len() >= self.batch_size {
                                self.flush_with_retry(&mut batch).await;
                            }
                        }
                        None => {
                            self.flush_with_retry(&mut batch).await;
                            return;
                        }
                    }
                }
                _ = flush_timer.tick(), if !batch.is_empty() => {
                    self.flush_with_retry(&mut batch).await;
                }
            }
        }
    }

    async fn flush_with_retry(&self, batch: &mut Vec<StoredBar>) {
        let mut retry_delay = Duration::from_secs(1);

        loop {
            match self.client.insert_batch(batch).await {
                Ok(()) => {
                    println!("Inserted {} bars into ClickHouse", batch.len());
                    batch.clear();
                    return;
                }
                Err(error) => {
                    eprintln!(
                        "ClickHouse batch insert failed: {error}; retrying in {}s",
                        retry_delay.as_secs()
                    );
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
                }
            }
        }
    }
}

struct ClickHouse {
    client: Client,
    url: String,
    database: String,
    user: String,
    password: String,
}

impl ClickHouse {
    async fn connect() -> Result<Self, ClickHouseError> {
        let clickhouse = Self {
            client: Client::new(),
            url: std::env::var("CLICKHOUSE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8123".to_owned()),
            database: std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "quant".to_owned()),
            user: std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "quant".to_owned()),
            password: std::env::var("CLICKHOUSE_PASSWORD")
                .unwrap_or_else(|_| "quantpass".to_owned()),
        };

        clickhouse.execute(CREATE_BARS_TABLE.to_owned()).await?;
        Ok(clickhouse)
    }

    async fn insert_batch(&self, bars: &[StoredBar]) -> Result<(), ClickHouseError> {
        if bars.is_empty() {
            return Ok(());
        }

        let mut query = String::from("INSERT INTO bars_1m FORMAT JSONEachRow\n");
        for bar in bars {
            query.push_str(&serde_json::to_string(bar)?);
            query.push('\n');
        }
        self.execute(query).await
    }

    async fn execute(&self, query: String) -> Result<(), ClickHouseError> {
        let response = self
            .client
            .post(&self.url)
            .query(&[("database", self.database.as_str())])
            .basic_auth(&self.user, Some(&self.password))
            .body(query)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("ClickHouse returned {status}: {}", response.text().await?).into());
        }
        Ok(())
    }
}

fn format_timestamp(ts: i64) -> Result<String, ClickHouseError> {
    Ok(DateTime::<Utc>::from_timestamp(ts, 0)
        .ok_or_else(|| format!("invalid bar timestamp: {ts}"))?
        .format("%Y-%m-%d %H:%M:%S")
        .to_string())
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_rows_have_null_microstructure_fields() {
        let row =
            StoredBar::historical(1333, 1, 1_700_000_000, 10.0, 12.0, 9.0, 11.0, 100).unwrap();
        let json = serde_json::to_value(row).unwrap();

        assert_eq!(json["source"], "historical");
        assert!(json["vwap"].is_null());
        assert!(json["spread_close"].is_null());
        assert!(json["open_interest_close"].is_null());
    }

    #[test]
    fn live_rows_have_enrichment_fields() {
        let row = StoredBar::try_from(Bar1m {
            security_id: 1333,
            exchange_segment: 1,
            ts: 1_700_000_000,
            open: 10.0,
            high: 12.0,
            low: 9.0,
            close: 11.0,
            volume: 100,
            trade_count: 4,
            avg_trade_size: 25.0,
            vwap: 10.5,
            atp_close: 10.4,
            buy_qty_close: 60,
            sell_qty_close: 40,
            buy_sell_imbalance: 0.2,
            bid_close: 10.9,
            ask_close: 11.0,
            spread_close: 0.1,
            bid_depth_close: 50,
            ask_depth_close: 25,
            depth_imbalance: 1.0 / 3.0,
            open_interest_open: 1_000,
            open_interest_close: 1_010,
            open_interest_change: 10,
            tick_count: 20,
            price_change: 1.0,
            range: 3.0,
            first_ltt: 1_700_000_001,
            last_ltt: 1_700_000_059,
        })
        .unwrap();
        let json = serde_json::to_value(row).unwrap();

        assert_eq!(json["source"], "live");
        assert_eq!(json["vwap"], 10.5);
        assert_eq!(json["open_interest_change"], 10);
    }
}
