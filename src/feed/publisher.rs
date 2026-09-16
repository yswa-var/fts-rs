use super::models::FullPacket;

pub struct RedisPublisher {
    client: redis::Client,
    ticks_maxlen: usize,
}

impl RedisPublisher {
    pub fn new(redis_url: &str) -> redis::RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        // Keep Redis as a bounded replay buffer rather than an unbounded tick archive.
        // Override this based on the observed peak tick rate and desired recovery window:
        // peak_ticks_per_second * recovery_window_seconds * 1.25.
        let ticks_maxlen = std::env::var("TICKS_MAXLEN")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(750_000);

        Ok(Self {
            client,
            ticks_maxlen,
        })
    }

    pub async fn publish_full(&self, tick: &FullPacket) -> redis::RedisResult<()> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;

        let stream = "ticks";

        let mut args: Vec<String> = vec![
            "security_id".into(),
            tick.security_id.to_string(),
            "exchange_segment".into(),
            tick.exchange_segment.to_string(),
            "ltp".into(),
            tick.ltp.to_string(),
            "last_trade_qty".into(),
            tick.last_trade_qty.to_string(),
            "last_trade_time".into(),
            tick.last_trade_time.to_string(),
            "avg_trade_price".into(),
            tick.avg_trade_price.to_string(),
            "volume".into(),
            tick.volume.to_string(),
            "total_sell_qty".into(),
            tick.total_sell_qty.to_string(),
            "total_buy_qty".into(),
            tick.total_buy_qty.to_string(),
            "open_interest".into(),
            tick.open_interest.to_string(),
            "highest_oi".into(),
            tick.highest_oi.to_string(),
            "lowest_oi".into(),
            tick.lowest_oi.to_string(),
            "day_open".into(),
            tick.day_open.to_string(),
            "day_close".into(),
            tick.day_close.to_string(),
            "day_high".into(),
            tick.day_high.to_string(),
            "day_low".into(),
            tick.day_low.to_string(),
        ];

        for (i, level) in tick.depth.iter().enumerate() {
            args.push(format!("depth_{}_bid_qty", i));
            args.push(level.bid_qty.to_string());

            args.push(format!("depth_{}_ask_qty", i));
            args.push(level.ask_qty.to_string());

            args.push(format!("depth_{}_bid_orders", i));
            args.push(level.bid_orders.to_string());

            args.push(format!("depth_{}_ask_orders", i));
            args.push(level.ask_orders.to_string());

            args.push(format!("depth_{}_bid_price", i));
            args.push(level.bid_price.to_string());

            args.push(format!("depth_{}_ask_price", i));
            args.push(level.ask_price.to_string());
        }

        let _: String = redis::cmd("XADD")
            .arg(&stream)
            .arg("MAXLEN")
            .arg("~")
            .arg(self.ticks_maxlen)
            .arg("*")
            .arg(args)
            .query_async(&mut connection)
            .await?;

        Ok(())
    }
}
