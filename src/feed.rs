use std::convert::TryInto;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
};
use std::time::Duration;

use tracing::{error, info, warn};
use tokio::time::sleep;
#[derive(Debug)]
pub struct TickerPacket {
    pub response_code: u8,
    pub message_length: u16,
    pub exchange_segment: u8,
    pub security_id: i32,

    pub ltp: f32,
    pub last_trade_time: i32,
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Instrument {
    #[serde(rename = "ExchangeSegment")]
    pub exchange_segment: String,

    #[serde(rename = "SecurityId")]
    pub security_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    #[serde(rename = "RequestCode")]
    pub request_code: u8,

    #[serde(rename = "InstrumentCount")]
    pub instrument_count: usize,

    #[serde(rename = "InstrumentList")]
    pub instrument_list: Vec<Instrument>,
}

pub fn parse_ticker(data: &[u8]) -> Result<TickerPacket, String> {
    // Ticker packet:
    //
    // 0..8   = response header
    // 8..12  = LTP
    // 12..16 = LTT
    //
    // Total = 17 bytes according to Dhan's packet layout.

    if data.len() < 16 {
        return Err(format!(
            "ticker packet too short: {} bytes",
            data.len()
        ));
    }

    let response_code = data[0];

    let message_length =
        u16::from_le_bytes(
            data[1..3]
                .try_into()
                .map_err(|_| "invalid message length")?,
        );

    let exchange_segment = data[3];

    let security_id =
        i32::from_le_bytes(
            data[4..8]
                .try_into()
                .map_err(|_| "invalid security id")?,
        );

    let ltp =
        f32::from_le_bytes(
            data[8..12]
                .try_into()
                .map_err(|_| "invalid LTP")?,
        );

    let last_trade_time =
        i32::from_le_bytes(
            data[12..16]
                .try_into()
                .map_err(|_| "invalid LTT")?,
        );

    Ok(TickerPacket {
        response_code,
        message_length,
        exchange_segment,
        security_id,
        ltp,
        last_trade_time,
    })
}

pub async fn run_feed(
    access_token: &str,
    client_id: &str,
    subscription: Subscription,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!(
        "wss://api-feed.dhan.co/?version=2&token={}&clientId={}&authType=2",
        access_token,
        client_id
    );

    let mut attempt = 0u32;

    loop {
        info!("Connecting...");

        match connect_async(&url).await {
            Ok((mut ws, _)) => {
                info!("Connected.");

                attempt = 0;

                let message = serde_json::to_string(&subscription)?;

                if let Err(err) = ws
                    .send(Message::Text(message.into()))
                    .await
                {
                    eprintln!("Failed to subscribe: {err}");
                    continue;
                }

                println!("Subscribed.");

                loop {
                    match ws.next().await {
                        Some(Ok(Message::Binary(data))) => {
                            match parse_ticker(&data) {
                                Ok(tick) => {
                                    println!(
                                        "SEC {:>6} | LTP {:>10.2} | LTT {}",
                                        tick.security_id,
                                        tick.ltp,
                                        tick.last_trade_time
                                    );
                                }

                                Err(err) => {
                                    eprintln!("Parse error: {err}");
                                }
                            }
                        }

                        Some(Ok(Message::Ping(data))) => {
                            if let Err(err) =
                                ws.send(Message::Pong(data)).await
                            {
                                eprintln!("Pong failed: {err}");
                                break;
                            }
                        }

                        Some(Ok(Message::Close(frame))) => {
                            println!("WebSocket closed: {frame:?}");
                            break;
                        }

                        Some(Ok(_)) => {}

                        Some(Err(err)) => {
                            eprintln!("WebSocket error: {err}");
                            break;
                        }

                        None => {
                            println!("WebSocket stream ended.");
                            break;
                        }
                    }
                }
            }

            Err(err) => {
                eprintln!("Connection failed: {err}");
            }
        }

        attempt += 1;

        let delay = reconnect_delay(attempt);

        println!(
            "Reconnecting in {} seconds...",
            delay.as_secs()
        );

        sleep(delay).await;
    }
}

fn reconnect_delay(attempt: u32) -> Duration {
    let seconds = 2u64.pow(attempt.min(5));

    Duration::from_secs(seconds.min(30))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ticker_packet() {
        let mut packet = vec![0u8; 16];

        // Response code = Ticker
        packet[0] = 2;

        // Message length
        packet[1..3].copy_from_slice(&17u16.to_le_bytes());

        // Exchange segment
        packet[3] = 1;

        // Security ID
        packet[4..8].copy_from_slice(&1333i32.to_le_bytes());

        // LTP
        packet[8..12].copy_from_slice(&1400.50f32.to_le_bytes());

        // LTT
        packet[12..16].copy_from_slice(&1773139200i32.to_le_bytes());

        let ticker = parse_ticker(&packet).unwrap();

        assert_eq!(ticker.response_code, 2);
        assert_eq!(ticker.security_id, 1333);
        assert!((ticker.ltp - 1400.50).abs() < 0.001);
        assert_eq!(ticker.last_trade_time, 1773139200);
    }
}
