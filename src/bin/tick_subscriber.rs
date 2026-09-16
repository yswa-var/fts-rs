use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};
use std::time::Instant;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut connection = client.get_multiplexed_async_connection().await?;

    let stream = "ticks";
    let group = "bar-aggregators";
    let consumer = "worker-1";

    // Create consumer group.
    // MKSTREAM creates `ticks` if it doesn't exist yet.
    let started = Instant::now();
    let result: redis::RedisResult<String> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream)
        .arg(group)
        .arg("0")
        .arg("MKSTREAM")
        .query_async(&mut connection)
        .await;

    println!("redis read took {:?}", started.elapsed());

    match result {
        Ok(_) => println!("Created consumer group: {group}"),

        Err(err) if err.to_string().contains("BUSYGROUP") => {
            println!("Consumer group already exists: {group}");
        }

        Err(err) => return Err(err),
    }

    println!("Listening to stream={stream}, group={group}, consumer={consumer}");

    let opts = StreamReadOptions::default()
        .group(group, consumer)
        .count(100);

    loop {
        let reply: StreamReadReply = match connection.xread_options(&[stream], &[">"], &opts).await
        {
            Ok(reply) => reply,

            Err(err) => {
                eprintln!(
                    "XREADGROUP error: kind={:?}, detail={:?}, full={}",
                    err.kind(),
                    err.detail(),
                    err
                );

                continue;
            }
        };
        for stream_key in reply.keys {
            for entry in stream_key.ids {
                println!("id: {}", entry.id);

                // for (_, value) in &entry.map {
                //     println!("{value:?}");
                // }

                // Acknowledge only AFTER processing succeeded.
                let _: usize = connection.xack(stream, group, &[&entry.id]).await?;
            }
        }
    }
}
