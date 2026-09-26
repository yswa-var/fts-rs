//! Browser UI for the live Redis tick stream.
//!
//! Run this alongside the feed producer with `cargo run --bin tick_frontend`,
//! then open http://127.0.0.1:3000.

use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};

use async_stream::stream;
use axum::{
    Router,
    response::{Html, Sse},
    routing::get,
};
use redis::{
    AsyncCommands,
    streams::{StreamReadOptions, StreamReadReply},
};
use serde::Serialize;

const REDIS_URL: &str = "redis://127.0.0.1/";
const STREAM: &str = "ticks";

#[derive(Clone)]
struct AppState {
    instruments: Arc<HashMap<String, String>>,
}

#[derive(Serialize)]
struct TickUpdate {
    security_id: String,
    instrument: String,
    ltp: f64,
    ltt: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = AppState {
        instruments: Arc::new(load_instruments("master.csv")?),
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/events", get(events))
        .with_state(state);
    let address: SocketAddr = "127.0.0.1:3000".parse()?;

    tracing::info!("Tick frontend listening on http://{address}");
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Maps each security ID to the `DISPLAY_NAME` column requested for the table.
fn load_instruments(path: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let security_id = header_index(&headers, "SECURITY_ID")?;
    let instrument = header_index(&headers, "DISPLAY_NAME")?;
    let mut result = HashMap::new();

    for record in reader.records() {
        let record = record?;
        let id = record.get(security_id).unwrap_or_default().trim();
        let name = record.get(instrument).unwrap_or_default().trim();
        if !id.is_empty() && !name.is_empty() {
            result.insert(id.to_owned(), name.to_owned());
        }
    }
    Ok(result)
}

fn header_index(
    headers: &csv::StringRecord,
    wanted: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    headers
        .iter()
        .position(|header| header.trim().eq_ignore_ascii_case(wanted))
        .ok_or_else(|| format!("master.csv is missing the {wanted} column").into())
}

async fn events(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let updates = stream! {
        let client = match redis::Client::open(REDIS_URL) {
            Ok(client) => client,
            Err(error) => {
                tracing::error!("Cannot create Redis client: {error}");
                return;
            }
        };
        let mut connection = match client.get_multiplexed_async_connection().await {
            Ok(connection) => connection,
            Err(error) => {
                tracing::error!("Cannot connect to Redis: {error}");
                return;
            }
        };
        // `$` means a newly opened page receives live ticks only, avoiding a
        // costly replay of the historical stream.
        let mut last_id = "$".to_owned();
        let options = StreamReadOptions::default().block(15_000).count(250);

        loop {
            let reply: redis::RedisResult<StreamReadReply> = connection
                .xread_options(&[STREAM], &[last_id.as_str()], &options)
                .await;
            let reply = match reply {
                Ok(reply) => reply,
                Err(error) => {
                    tracing::warn!("Redis stream read failed: {error}");
                    continue;
                }
            };

            for key in reply.keys {
                for entry in key.ids {
                    last_id = entry.id;
                    let security_id = stream_value(&entry.map, "security_id");
                    let ltp = stream_value(&entry.map, "ltp").parse::<f64>().ok();
                    if security_id.is_empty() || ltp.is_none() {
                        continue;
                    }
                    let update = TickUpdate {
                        instrument: state.instruments.get(&security_id)
                            .cloned()
                            .unwrap_or_else(|| format!("Unknown ({security_id})")),
                        security_id,
                        ltp: ltp.unwrap(),
                        ltt: stream_value(&entry.map, "last_trade_time"),
                    };
                    if let Ok(data) = serde_json::to_string(&update) {
                        yield Ok(axum::response::sse::Event::default().data(data));
                    }
                }
            }
        }
    };
    Sse::new(updates)
}

fn stream_value(map: &std::collections::HashMap<String, redis::Value>, key: &str) -> String {
    map.get(key)
        .and_then(|value| redis::from_redis_value::<String>(value.clone()).ok())
        .unwrap_or_default()
}

async fn index() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Live ticks</title><style>
body{font-family:system-ui,sans-serif;background:#f8fafc;color:#172033;margin:2rem}table{border-collapse:collapse;background:white;min-width:540px;box-shadow:0 1px 3px #0002}th,td{padding:.6rem .85rem;text-align:left;border-bottom:1px solid #e5e7eb}th{background:#172033;color:white}td.price{font-variant-numeric:tabular-nums;transition:background-color .25s}td.up{background:#bbf7d0}td.down{background:#fecaca}.status{margin-bottom:1rem;color:#526075}
</style></head><body><h1>Live ticks</h1><p class="status" id="status">Connecting to Redis stream…</p>
<table><thead><tr><th>Instrument</th><th>Security ID</th><th>LTT</th><th>LTP</th></tr></thead><tbody id="ticks"></tbody></table>
<script>
const rows=new Map(), source=new EventSource('/events'), status=document.querySelector('#status');
const formatLtt=value=>{const seconds=Number(value);return Number.isFinite(seconds)&&seconds>0?new Date(seconds*1000).toLocaleTimeString():value};
source.onopen=()=>status.textContent='Connected — waiting for ticks';
source.onerror=()=>status.textContent='Connection interrupted — retrying…';
source.onmessage=event=>{const tick=JSON.parse(event.data), key=tick.security_id;let row=rows.get(key);
 if(!row){row=document.createElement('tr');row.innerHTML='<td></td><td></td><td></td><td class="price"></td>';rows.set(key,row);document.querySelector('#ticks').append(row)}
 const cells=row.cells, price=cells[3], previous=Number(price.dataset.value);cells[0].textContent=tick.instrument;cells[1].textContent=tick.security_id;cells[2].textContent=formatLtt(tick.ltt);price.textContent=tick.ltp;price.dataset.value=tick.ltp;price.className='price';
 if(!Number.isNaN(previous)) price.classList.add(tick.ltp>previous?'up':tick.ltp<previous?'down':'');
 status.textContent='Live updates received';};
</script></body></html>"#,
    )
}
