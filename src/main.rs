mod auth;
mod hdata;
mod master;
mod models;
mod quote;
mod feed;

use auth::get_access_token;
use hdata::{Symbol, fetch_symbols};
use reqwest::Client;
use serde_json::json;
use feed::{Instrument, Subscription};

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
        request_code: 15,
        instrument_count: 3,
        instrument_list: vec![
            Instrument {
                exchange_segment: "NSE_EQ".into(),
                security_id: "18143".into(),
            },
            Instrument {
                exchange_segment: "NSE_EQ".into(),
                security_id: "21740".into(),
            },
            Instrument {
                exchange_segment: "NSE_EQ".into(),
                security_id: "13611".into(),
            },
        ],
    };

    feed::run_feed(
        &access_token,
        &client_id,
        subscription,
    ).await?;
    Ok(())
}
