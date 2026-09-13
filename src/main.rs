mod auth;
mod feed;
mod hdata;
mod master;
mod models;
mod quote;

use auth::get_access_token;
use feed::{Instrument, Subscription};
use hdata::{Symbol, fetch_symbols};
use reqwest::Client;
use serde_json::json;

const MCX: &str = "MCX_COMM";
const NSEEQ: &str = "NSE_EQ";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let (access_token, client_id) = get_access_token().await?;

    let client = Client::new();

    // let payload = json!({
    //     "NSE_EQ": [11536, 1038],
    //     "BSE_EQ": [532540]
    // });

    // let quote_response = quote::get_quote(&client, &access_token, &client_id, &payload).await?;
    // println!("{:#?}", quote_response);

    // let symbols = [Symbol {
    //     name: "CHOLAHLDNG",
    //     security_id: "21740",
    //     exchange_segment: "NSE_EQ",
    //     instrument: "EQUITY",
    // }];

    // fetch_symbols(&client, &access_token, &symbols).await?;

    let subscription = Subscription {
        request_code: 21,
        instrument_count: 4,
        instrument_list: vec![
            Instrument {
                exchange_segment: NSEEQ.into(),
                security_id: "2475".into(),
            },
            Instrument {
                exchange_segment: NSEEQ.into(),
                security_id: "3787".into(),
            },
            Instrument {
                exchange_segment: NSEEQ.into(),
                security_id: "1624".into(),
            },
            Instrument {
                exchange_segment: NSEEQ.into(),
                security_id: "4668".into(),
            },
        ],
    };

    feed::run(&access_token, &client_id, subscription).await?;
    Ok(())
}
