use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct QuoteResponse {
    pub status: String,
    pub data: HashMap<String, HashMap<String, Quote>>,
}

#[derive(Debug, Deserialize)]
pub struct Quote {
    pub average_price: f64,
    pub buy_quantity: i64,
    pub sell_quantity: i64,
    pub last_price: f64,
    pub last_quantity: i64,
    pub last_trade_time: String,
    pub lower_circuit_limit: f64,
    pub upper_circuit_limit: f64,
    pub net_change: f64,
    pub volume: i64,
    pub oi: i64,

    // Keep this flexible initially.
    pub depth: Option<Value>,
}

pub async fn get_quote(
    client: &Client,
    access_token: &str,
    client_id: &str,
    payload: &Value,
) -> Result<QuoteResponse, Box<dyn std::error::Error>> {
    let response = client
        .post("https://api.dhan.co/v2/marketfeed/quote")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("access-token", access_token)
        .header("client-id", client_id)
        .json(payload)
        .send()
        .await?
        .error_for_status()?;

    let quote_response: QuoteResponse = response.json().await?;

    Ok(quote_response)
}
