//! HTTP worker that backfills Dhan historical candles into ClickHouse.

#[path = "../auth.rs"]
mod auth;
#[allow(unused_imports)]
#[path = "../feed/mod.rs"]
mod feed;
#[path = "../hdata.rs"]
mod hdata;
#[path = "../master.rs"]
mod master;
#[path = "../models.rs"]
mod models;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use hdata::{Candle, Timeframe};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, error::Error, net::SocketAddr, sync::Arc};

type WorkerError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct AppState {
    dhan: Client,
    access_token: Arc<String>,
    clickhouse: Arc<ClickHouse>,
}

#[derive(Debug, Deserialize)]
struct BackfillRequest {
    security_ids: Vec<String>,
    timeframe: String,
}

#[derive(Debug, Serialize)]
struct BackfillResponse {
    timeframe: String,
    results: Vec<BackfillResult>,
}

#[derive(Debug, Serialize)]
struct BackfillResult {
    security_id: String,
    status: &'static str,
    from_date: Option<String>,
    to_date: Option<String>,
    candles_fetched: usize,
    candles_written: usize,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct HistoricalBar {
    security_id: u64,
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: i64,
}

#[derive(Debug, Deserialize)]
struct LatestRow {
    latest: Option<i64>,
}

struct ClickHouse {
    client: Client,
    url: String,
    database: String,
    user: String,
    password: String,
}

impl ClickHouse {
    async fn connect_from_env() -> Result<Self, WorkerError> {
        let clickhouse = Self {
            client: Client::new(),
            url: env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".to_owned()),
            database: env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "quant".to_owned()),
            user: env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "quant".to_owned()),
            password: env::var("CLICKHOUSE_PASSWORD").unwrap_or_else(|_| "quantpass".to_owned()),
        };
        clickhouse.execute("SELECT 1").await?;
        clickhouse.create_tables().await?;
        Ok(clickhouse)
    }

    async fn create_tables(&self) -> Result<(), WorkerError> {
        for table in ["bar_1m", "bar_daily"] {
            self.execute(&format!(
                "CREATE TABLE IF NOT EXISTS {table} (\
                    security_id UInt64, \
                    timestamp Int64, \
                    open Float64, \
                    high Float64, \
                    low Float64, \
                    close Float64, \
                    volume Int64\
                 ) ENGINE = ReplacingMergeTree ORDER BY (security_id, timestamp)"
            ))
            .await?;
        }
        Ok(())
    }

    async fn latest_timestamp(
        &self,
        table: &str,
        security_id: u64,
    ) -> Result<Option<i64>, WorkerError> {
        let query = format!(
            "SELECT maxOrNull(timestamp) AS latest FROM {table} WHERE security_id = {security_id} FORMAT JSONEachRow"
        );
        let body = self.query(&query).await?;
        if body.trim().is_empty() {
            return Ok(None);
        }
        Ok(serde_json::from_str::<LatestRow>(body.trim())?.latest)
    }

    async fn insert(&self, table: &str, bars: &[HistoricalBar]) -> Result<(), WorkerError> {
        if bars.is_empty() {
            return Ok(());
        }
        let mut body = format!("INSERT INTO {table} FORMAT JSONEachRow\n");
        for bar in bars {
            body.push_str(&serde_json::to_string(bar)?);
            body.push('\n');
        }
        self.execute(&body).await
    }

    async fn execute(&self, query: &str) -> Result<(), WorkerError> {
        self.request(query).await.map(|_| ())
    }

    async fn query(&self, query: &str) -> Result<String, WorkerError> {
        self.request(query).await
    }

    async fn request(&self, body: &str) -> Result<String, WorkerError> {
        let response = self
            .client
            .post(&self.url)
            .query(&[("database", self.database.as_str())])
            .basic_auth(&self.user, Some(&self.password))
            .body(body.to_owned())
            .send()
            .await?;
        let status = response.status();
        let response_body = response.text().await?;
        if !status.is_success() {
            return Err(format!("ClickHouse returned {status}: {response_body}").into());
        }
        Ok(response_body)
    }
}

#[tokio::main]
async fn main() -> Result<(), WorkerError> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter("info").init();

    let clickhouse = ClickHouse::connect_from_env().await?;
    let (access_token, _) = auth::get_access_token()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let state = AppState {
        dhan: Client::new(),
        access_token: Arc::new(access_token),
        clickhouse: Arc::new(clickhouse),
    };
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/historical/backfill", post(backfill))
        .with_state(state);
    let address: SocketAddr = env::var("HISTORICAL_WORKER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3001".to_owned())
        .parse()?;
    tracing::info!(%address, "historical worker listening");
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn backfill(
    State(state): State<AppState>,
    Json(request): Json<BackfillRequest>,
) -> impl IntoResponse {
    let timeframe = match parse_timeframe(&request.timeframe) {
        Ok(timeframe) => timeframe,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(error_response(request.timeframe, error)),
            );
        }
    };
    if request.security_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(error_response(
                request.timeframe,
                "security_ids must not be empty".to_owned(),
            )),
        );
    }

    let mut results = Vec::with_capacity(request.security_ids.len());
    for security_id in request.security_ids {
        results.push(backfill_symbol(&state, timeframe, security_id).await);
    }
    (
        StatusCode::OK,
        Json(BackfillResponse {
            timeframe: request.timeframe,
            results,
        }),
    )
}

fn error_response(timeframe: String, error: String) -> BackfillResponse {
    BackfillResponse {
        timeframe,
        results: vec![BackfillResult {
            security_id: String::new(),
            status: "error",
            from_date: None,
            to_date: None,
            candles_fetched: 0,
            candles_written: 0,
            error: Some(error),
        }],
    }
}

async fn backfill_symbol(
    state: &AppState,
    timeframe: Timeframe,
    security_id: String,
) -> BackfillResult {
    let symbol = match master::historical_symbol(&security_id) {
        Ok(symbol) => symbol,
        Err(error) => return failed(security_id, error.to_string(), None, None),
    };
    let numeric_id = match symbol.security_id.parse::<u64>() {
        Ok(id) => id,
        Err(_) => {
            return failed(
                security_id,
                "security_id must be an unsigned integer".to_owned(),
                None,
                None,
            );
        }
    };
    let table = table_for(timeframe);
    let latest = match state.clickhouse.latest_timestamp(table, numeric_id).await {
        Ok(latest) => latest,
        Err(error) => return failed(security_id, error.to_string(), None, None),
    };
    let now = Utc::now().with_timezone(&ist());
    let from = latest
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        .map(|value| value.with_timezone(&ist()))
        .unwrap_or_else(|| initial_start(now, timeframe));
    let (from_date, to_date) = display_dates(from, now, timeframe);

    let candles = match fetch_range(state, &symbol, timeframe, from, now).await {
        Ok(candles) => candles,
        Err(error) => {
            return failed(
                security_id,
                error.to_string(),
                Some(from_date),
                Some(to_date),
            );
        }
    };
    let bars = candles
        .iter()
        .map(|candle| HistoricalBar {
            security_id: numeric_id,
            timestamp: candle.timestamp,
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            volume: candle.volume,
        })
        .collect::<Vec<_>>();
    if let Err(error) = state.clickhouse.insert(table, &bars).await {
        return failed(
            security_id,
            error.to_string(),
            Some(from_date),
            Some(to_date),
        );
    }
    BackfillResult {
        security_id,
        status: "ok",
        from_date: Some(from_date),
        to_date: Some(to_date),
        candles_fetched: candles.len(),
        candles_written: bars.len(),
        error: None,
    }
}

async fn fetch_range(
    state: &AppState,
    symbol: &hdata::Symbol,
    timeframe: Timeframe,
    from: DateTime<FixedOffset>,
    now: DateTime<FixedOffset>,
) -> Result<Vec<Candle>, Box<dyn Error>> {
    match timeframe {
        Timeframe::Daily => {
            let (from_date, to_date) = display_dates(from, now, timeframe);
            hdata::get_historical(
                &state.dhan,
                &state.access_token,
                symbol,
                &from_date,
                &to_date,
            )
            .await
        }
        Timeframe::OneMinute => {
            let mut candles = Vec::new();
            for (start, end) in intraday_windows(from, now) {
                let chunk = hdata::get_intraday(
                    &state.dhan,
                    &state.access_token,
                    symbol,
                    &start.format("%Y-%m-%d %H:%M:%S").to_string(),
                    &end.format("%Y-%m-%d %H:%M:%S").to_string(),
                )
                .await?;
                candles.extend(chunk);
            }
            Ok(candles)
        }
    }
}

fn parse_timeframe(value: &str) -> Result<Timeframe, String> {
    match value {
        "1m" => Ok(Timeframe::OneMinute),
        "daily" => Ok(Timeframe::Daily),
        _ => Err("timeframe must be either 1m or daily".to_owned()),
    }
}

fn table_for(timeframe: Timeframe) -> &'static str {
    match timeframe {
        Timeframe::OneMinute => "bar_1m",
        Timeframe::Daily => "bar_daily",
    }
}

fn ist() -> FixedOffset {
    FixedOffset::east_opt(5 * 60 * 60 + 30 * 60).expect("IST is a valid offset")
}

fn initial_start(now: DateTime<FixedOffset>, timeframe: Timeframe) -> DateTime<FixedOffset> {
    now - Duration::days(match timeframe {
        Timeframe::OneMinute => 365 * 5,
        Timeframe::Daily => 365 * 10,
    })
}

fn display_dates(
    from: DateTime<FixedOffset>,
    now: DateTime<FixedOffset>,
    timeframe: Timeframe,
) -> (String, String) {
    match timeframe {
        Timeframe::OneMinute => (
            from.format("%Y-%m-%d %H:%M:%S").to_string(),
            now.format("%Y-%m-%d %H:%M:%S").to_string(),
        ),
        // Dhan daily `toDate` is non-inclusive, so tomorrow includes today's candle.
        Timeframe::Daily => (
            from.format("%Y-%m-%d").to_string(),
            (now.date_naive() + Duration::days(1))
                .format("%Y-%m-%d")
                .to_string(),
        ),
    }
}

fn intraday_windows(
    from: DateTime<FixedOffset>,
    to: DateTime<FixedOffset>,
) -> Vec<(DateTime<FixedOffset>, DateTime<FixedOffset>)> {
    let mut windows = Vec::new();
    let mut start = from;
    while start < to {
        let end = (start + Duration::days(90)).min(to);
        windows.push((start, end));
        start = end;
    }
    windows
}

fn failed(
    security_id: String,
    error: String,
    from_date: Option<String>,
    to_date: Option<String>,
) -> BackfillResult {
    BackfillResult {
        security_id,
        status: "error",
        from_date,
        to_date,
        candles_fetched: 0,
        candles_written: 0,
        error: Some(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_supported_timeframes_are_accepted() {
        assert_eq!(parse_timeframe("1m").unwrap(), Timeframe::OneMinute);
        assert_eq!(parse_timeframe("daily").unwrap(), Timeframe::Daily);
        assert!(parse_timeframe("5m").is_err());
    }

    #[test]
    fn intraday_ranges_are_limited_to_ninety_days() {
        let start = DateTime::parse_from_rfc3339("2020-01-01T09:15:00+05:30").unwrap();
        let end = start + Duration::days(181);
        let windows = intraday_windows(start, end);
        assert_eq!(windows.len(), 3);
        assert!(
            windows
                .iter()
                .all(|(from, to)| *to - *from <= Duration::days(90))
        );
        assert_eq!(windows[0].0, start);
        assert_eq!(windows.last().unwrap().1, end);
    }

    #[test]
    fn daily_end_date_includes_the_current_day() {
        let now = DateTime::parse_from_rfc3339("2026-09-22T12:00:00+05:30").unwrap();
        let (from, to) = display_dates(now - Duration::days(1), now, Timeframe::Daily);
        assert_eq!(from, "2026-09-21");
        assert_eq!(to, "2026-09-23");
    }
}
