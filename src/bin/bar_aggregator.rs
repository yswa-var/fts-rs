#[path = "../bar_aggregator/mod.rs"]
mod bar_aggregator;

use bar_aggregator::{BarAggregator, Tick};
use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
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
                            println!("CANDLE {bar:#?}");
                        }
                    }
                    Err(error) => eprintln!("Skipping malformed tick {}: {error}", entry.id),
                }
                let _: usize = connection.xack(stream, group, &[&entry.id]).await?;
            }
        }
    }
}
