use redis::AsyncCommands;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let client = redis::Client::open("redis://127.0.0.1/")?;

    let mut connection = client.get_multiplexed_async_connection().await?;

    let stream = "tick:4668";

    println!("Listening to {stream}");

    loop {
        let result: redis::Value = redis::cmd("XREAD")
            .arg("BLOCK")
            .arg("0")
            .arg("STREAMS")
            .arg(stream)
            .arg("0")
            .query_async(&mut connection)
            .await?;

        println!("{result:?}");
    }
}
