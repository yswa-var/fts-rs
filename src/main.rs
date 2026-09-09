mod auth;
mod master;
mod models;
mod quote;

use auth::get_access_token;
// use master::get_master;
use serde_json::json;
// use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let (access_token, client_id) = get_access_token()?;
    // get_master()?;

    let payload = json!({
        "NSE_EQ": [11536, 1038],
        "BSE_EQ": [532540]
    });

    let quote_response = quote::get_quote(&access_token, &client_id, &payload).await?;

    println!("{:#?}", quote_response);

    Ok(())
}
