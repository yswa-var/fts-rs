#[path = "../bar_aggregator/mod.rs"]
mod bar_aggregator;

use bar_aggregator::{BarAggregator, ClickHouseWriter, StoredBar, Tick};
use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    dotenvy::dotenv().ok();
    let bar_sender = ClickHouseWriter::start_from_env()
        .await
        .map_err(clickhouse_error)?;
    println!("ClickHouse batch writer started and bars_1m is ready");

    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut connection = client.get_multiplexed_async_connection().await?;

    let stream = "ticks";
    let group = "bar-aggregators";
    let consumer = "bar-worker-1";

    let result: redis::RedisResult<String> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream)
        .arg(group)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(&mut connection)
        .await;
    match result {
        Ok(_) => println!("Created consumer group: {group}"),
        Err(error) if error.to_string().contains("BUSYGROUP") => {}
        Err(error) => return Err(error),
    }

    println!("Listening to stream={stream}, group={group}, consumer={consumer}");
    let options = StreamReadOptions::default()
        .group(group, consumer)
        .count(100);
    let mut aggregator = BarAggregator::new();

    loop {
        let reply: StreamReadReply = connection
            .xread_options(&[stream], &[">"], &options)
            .await?;
        for stream_key in reply.keys {
            for entry in stream_key.ids {
                match Tick::from_stream_entry(&entry.map) {
                    Ok(tick) => {
                        if let Some(bar) = aggregator.on_tick(tick) {
                            // println!("CANDLE {bar:#?}");
                            let row = StoredBar::try_from(bar).map_err(clickhouse_error)?;
                            bar_sender.send(row).await.map_err(|_| {
                                redis::RedisError::from((
                                    redis::ErrorKind::Io,
                                    "ClickHouse writer stopped",
                                ))
                            })?;
                        }
                    }
                    Err(error) => eprintln!("Skipping malformed tick {}: {error}", entry.id),
                }
                let _: usize = connection.xack(stream, group, &[&entry.id]).await?;
            }
        }
    }
}

fn clickhouse_error(error: Box<dyn std::error::Error + Send + Sync>) -> redis::RedisError {
    redis::RedisError::from((
        redis::ErrorKind::Io,
        "ClickHouse operation failed",
        error.to_string(),
    ))
}
