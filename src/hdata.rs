use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tokio::time::{Duration as TokioDuration, sleep};

const HISTORICAL_URL: &str = "https://api.dhan.co/v2/charts/historical";

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: &'static str,
    pub security_id: &'static str,
    pub exchange_segment: &'static str,
    pub instrument: &'static str,
}

#[derive(Debug, Deserialize)]
struct HistoricalResponse {
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<f64>,
    timestamp: Vec<f64>,

    #[serde(default)]
    open_interest: Vec<f64>,
}

#[derive(Debug, Serialize)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub open_interest: i64,
}

pub async fn get_historical(
    client: &Client,
    access_token: &str,
    symbol: &Symbol,
    from_date: &str,
    to_date: &str,
) -> Result<Vec<Candle>, Box<dyn std::error::Error>> {
    let payload = serde_json::json!({
        "securityId": symbol.security_id,
        "exchangeSegment": symbol.exchange_segment,
        "instrument": symbol.instrument,
        "expiryCode": 0,
        "oi": false,
        "fromDate": from_date,
        "toDate": to_date,
    });

    let response = client
        .post(HISTORICAL_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("access-token", access_token)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    let data: HistoricalResponse = response.json().await?;

    let len = data
        .open
        .len()
        .min(data.high.len())
        .min(data.low.len())
        .min(data.close.len())
        .min(data.volume.len())
        .min(data.timestamp.len());

    let mut candles = Vec::with_capacity(len);

    for i in 0..len {
        candles.push(Candle {
            timestamp: data.timestamp[i] as i64,
            open: data.open[i],
            high: data.high[i],
            low: data.low[i],
            close: data.close[i],
            volume: data.volume[i] as i64,
            open_interest: data.open_interest.get(i).copied().unwrap_or(0.0) as i64,
        });
    }

    Ok(candles)
}

pub async fn fetch_symbols(
    client: &Client,
    access_token: &str,
    symbols: &[Symbol],
) -> Result<(), Box<dyn std::error::Error>> {
    let today = Utc::now().date_naive();

    // Six months ago.
    let from_date = today - Duration::days(183);

    let from_date = from_date.format("%Y-%m-%d").to_string();
    let to_date = today.format("%Y-%m-%d").to_string();

    fs::create_dir_all("data")?;

    for (i, symbol) in symbols.iter().enumerate() {
        println!(
            "[{}/{}] Fetching {}: {} -> {}",
            i + 1,
            symbols.len(),
            symbol.name,
            from_date,
            to_date
        );

        let candles = get_historical(client, access_token, symbol, &from_date, &to_date).await?;

        let path = format!("data/{}_daily.json", symbol.name);

        let json = serde_json::to_string_pretty(&candles)?;
        fs::write(&path, json)?;

        println!("  saved {} candles -> {}", candles.len(), path);

        // Don't hammer the API when batching symbols.
        if i + 1 < symbols.len() {
            sleep(TokioDuration::from_millis(500)).await;
        }
    }

    Ok(())
}
