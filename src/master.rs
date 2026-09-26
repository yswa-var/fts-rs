use crate::{feed::Instrument, hdata::Symbol};
use csv::StringRecord;
use reqwest::blocking::Client;
use std::{collections::HashMap, fs, path::Path};
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

/// Resolves a Dhan security ID to the additional fields required by its
/// historical-data endpoints. IDs mapping to more than one instrument are
/// rejected rather than silently choosing an exchange.
pub fn historical_symbol(security_id: &str) -> Result<Symbol, Box<dyn std::error::Error>> {
    historical_symbol_at(Path::new("master.csv"), security_id)
}

fn historical_symbol_at(
    path: &Path,
    security_id: &str,
) -> Result<Symbol, Box<dyn std::error::Error>> {
    let wanted = security_id.trim();
    if wanted.is_empty() || !wanted.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("invalid security_id: {security_id}").into());
    }

    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let exch_id = column_index(&headers, "EXCH_ID")?;
    let segment = column_index(&headers, "SEGMENT")?;
    let security_id_column = column_index(&headers, "SECURITY_ID")?;
    let instrument_column = column_index(&headers, "INSTRUMENT")?;
    let display_name = column_index(&headers, "DISPLAY_NAME")?;
    let mut matches = HashMap::new();

    for row in reader.records() {
        let row = row?;
        if row.get(security_id_column).unwrap_or_default().trim() != wanted {
            continue;
        }
        let exchange_segment = dhan_exchange_segment(
            row.get(exch_id).unwrap_or_default(),
            row.get(segment).unwrap_or_default(),
        )?;
        let instrument = row.get(instrument_column).unwrap_or_default().trim();
        if instrument.is_empty() {
            return Err(format!("security_id {wanted} has no instrument type").into());
        }
        matches.insert(
            (exchange_segment.clone(), instrument.to_owned()),
            Symbol {
                name: row.get(display_name).unwrap_or_default().trim().to_owned(),
                security_id: wanted.to_owned(),
                exchange_segment,
                instrument: instrument.to_owned(),
            },
        );
    }

    match matches.len() {
        0 => Err(format!("security_id {wanted} was not found in master.csv").into()),
        1 => Ok(matches.into_values().next().expect("one symbol")),
        _ => Err(format!("security_id {wanted} is ambiguous in master.csv").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_resolution_rejects_missing_and_ambiguous_ids() {
        let path = std::env::temp_dir().join(format!(
            "fts-rs-master-resolution-{}.csv",
            std::process::id()
        ));
        fs::write(
            &path,
            "EXCH_ID,SEGMENT,SECURITY_ID,INSTRUMENT,DISPLAY_NAME\nNSE,E,42,EQUITY,Answer\nNSE,E,7,EQUITY,One\nBSE,E,7,EQUITY,Two\n",
        )
        .unwrap();

        let symbol = historical_symbol_at(&path, "42").unwrap();
        assert_eq!(symbol.exchange_segment, "NSE_EQ");
        assert_eq!(symbol.instrument, "EQUITY");
        assert!(historical_symbol_at(&path, "404").is_err());
        assert!(historical_symbol_at(&path, "7").is_err());
        fs::remove_file(path).unwrap();
    }
}

fn column_index(headers: &StringRecord, name: &str) -> Result<usize, Box<dyn std::error::Error>> {
    headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| format!("master.csv is missing the {name} column").into())
}

fn dhan_exchange_segment(
    exchange: &str,
    segment: &str,
) -> Result<String, Box<dyn std::error::Error>> {
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
