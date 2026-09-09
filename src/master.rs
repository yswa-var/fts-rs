use reqwest::blocking::Client;
use std::fs;
use tracing::info;

pub fn get_master() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://images.dhan.co/api-data/api-scrip-master-detailed.csv";

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let response = client.get(url).send()?.error_for_status()?;

    let data = response.bytes()?;

    fs::write("master.csv", data)?;

    info!("Saved master.csv");

    Ok(())
}
