use reqwest::blocking::Client;
use crate::feed::Instrument;
use csv::StringRecord;
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

/// Reads instruments whose `tag` column contains `tag` as a comma-separated value.
pub fn instruments_for_tag(tag: &str) -> Result<Vec<Instrument>, Box<dyn std::error::Error>> {
    let wanted = tag.trim();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }

    let mut reader = csv::Reader::from_path("master.csv")?;
    let headers = reader.headers()?.clone();
    let exch_id = column_index(&headers, "EXCH_ID")?;
    let segment = column_index(&headers, "SEGMENT")?;
    let security_id = column_index(&headers, "SECURITY_ID")?;
    let tag_column = column_index(&headers, "tag")?;

    let mut instruments = Vec::new();
    for row in reader.records() {
        let row = row?;
        let has_tag = row
            .get(tag_column)
            .unwrap_or_default()
            .split(',')
            .any(|value| value.trim().eq_ignore_ascii_case(wanted));

        if has_tag {
            let exchange_segment = dhan_exchange_segment(
                row.get(exch_id).unwrap_or_default(),
                row.get(segment).unwrap_or_default(),
            )?;
            instruments.push(Instrument {
                exchange_segment,
                security_id: row.get(security_id).unwrap_or_default().to_owned(),
            });
        }
    }

    Ok(instruments)
}

fn column_index(headers: &StringRecord, name: &str) -> Result<usize, Box<dyn std::error::Error>> {
    headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| format!("master.csv is missing the {name} column").into())
}

fn dhan_exchange_segment(exchange: &str, segment: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = match (exchange.trim(), segment.trim()) {
        ("NSE", "E") => "NSE_EQ",
        ("NSE", "D") => "NSE_FNO",
        ("NSE", "C") => "NSE_CURRENCY",
        ("BSE", "E") => "BSE_EQ",
        ("BSE", "D") => "BSE_FNO",
        ("BSE", "C") => "BSE_CURRENCY",
        ("MCX", "M") => "MCX_COMM",
        _ => return Err(format!("unsupported exchange segment: {exchange}/{segment}").into()),
    };
    Ok(value.into())
}
