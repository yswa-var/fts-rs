use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_tungstenite::{client_async_tls, tungstenite::Message};
use tracing::{error, info};

use super::{models::Subscription, parser::parse_full, publisher::RedisPublisher, reconnect};

/// Runs the resilient Dhan-to-Redis ingestion loop.
///
/// The loop establishes an authenticated WebSocket connection, subscribes to
/// the requested instruments, processes incoming market-data frames, and
/// reconnects with backoff whenever the feed ends or a connection fails. It is
/// intended to run for the lifetime of the feed process.
pub async fn run(
    access_token: &str,
    client_id: &str,
    subscription: Subscription,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!(
        "wss://api-feed.dhan.co/?version=2&token={}&clientId={}&authType=2",
        access_token, client_id
    );

    let publisher = RedisPublisher::new("redis://127.0.0.1/")?;

    let mut attempt = 0;

    loop {
        info!("Connecting to Dhan feed...");

        match connect(&url).await {
            Ok((mut ws, _response)) => {
                info!("Connected");

                attempt = 0;

                subscribe(&mut ws, &subscription).await?;
                info!("Starting receive loop");

                if let Err(error) = receive_messages(&mut ws, &publisher).await {
                    error!("Feed error: {error}");
                }
            }

            Err(error) => {
                error!("Connection failed: {error}");
            }
        }

        attempt += 1;

        let delay = reconnect::delay(attempt);

        info!("Reconnecting in {} seconds", delay.as_secs());

        sleep(delay).await;
    }
}
/// Opens a TLS WebSocket connection to Dhan using an IPv4 address.
///
/// Resolving and dialing IPv4 explicitly avoids an unavailable IPv6 route from
/// stalling feed startup. Both the TCP and WebSocket handshakes are bounded by
/// short timeouts so the outer loop can retry promptly.
async fn connect(
    url: &str,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    Box<dyn std::error::Error>,
> {
    let parsed = url::Url::parse(url)?;

    let host = parsed.host_str().ok_or("missing host")?;

    let port = parsed.port_or_known_default().ok_or("missing port")?;

    let ipv4 = tokio::net::lookup_host((host, port))
        .await?
        .find(|addr| addr.is_ipv4())
        .ok_or("no IPv4 address found")?;

    info!("Connecting to {ipv4}");

    let tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(ipv4)).await??;

    tcp.set_nodelay(true)?;

    let ws = tokio::time::timeout(Duration::from_secs(5), client_async_tls(url, tcp)).await??;

    Ok(ws)
}

/// Sends the requested instrument subscription in Dhan-sized batches.
///
/// Dhan accepts at most 100 instruments per request, so a large subscription
/// is split into independent messages while preserving its request code.
async fn subscribe(
    ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    subscription: &Subscription,
) -> Result<(), Box<dyn std::error::Error>> {
    for instruments in subscription.instrument_list.chunks(100) {
        let chunk = Subscription {
            request_code: subscription.request_code,
            instrument_count: instruments.len(),
            instrument_list: instruments.to_vec(),
        };

        let message = serde_json::to_string(&chunk)?;

        ws.send(Message::Text(message.into())).await?;
    }

    info!(
        "Subscribed {} instruments in {} request(s)",
        subscription.instrument_list.len(),
        subscription.instrument_list.len().div_ceil(100)
    );

    Ok(())
}

/// Receives WebSocket frames until the feed closes or returns an error.
///
/// Binary frames are passed through the decode-and-publish path, while ping
/// frames are answered to keep the connection alive. A clean close returns to
/// the caller so the outer run loop can reconnect.
async fn receive_messages(
    ws: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    publisher: &RedisPublisher,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match ws.next().await {
            // Some(Ok(Message::Binary(data))) => {
            //     info!("Received binary frame: {} bytes", data.len());

            //     if let Err(error) = handle_binary_message(&data, publisher).await {
            //         error!("Failed to process tick: {error}");
            //     }
            // }
            Some(Ok(Message::Binary(data))) => {
                if let Err(error) = handle_binary_message(&data, publisher).await {
                    error!("Failed to process tick: {error}");
                }
            }

            Some(Ok(Message::Ping(data))) => {
                ws.send(Message::Pong(data)).await?;
            }

            Some(Ok(Message::Close(frame))) => {
                info!("WebSocket closed: {frame:?}");
                break;
            }

            Some(Ok(_)) => {
                // Ignore messages we don't currently care about.
            }

            Some(Err(error)) => {
                return Err(error.into());
            }

            None => {
                info!("WebSocket stream ended");
                break;
            }
        }
    }

    Ok(())
}
/// Decodes one full-depth Dhan packet and appends it to the Redis tick stream.
///
/// Parse or Redis failures are returned to the receive loop, which logs the
/// bad frame and continues consuming later ticks when possible.
async fn handle_binary_message(
    data: &[u8],
    publisher: &RedisPublisher,
) -> Result<(), Box<dyn std::error::Error>> {
    let tick = parse_full(data)?;

    publisher.publish_full(&tick).await?;

    Ok(())
}
