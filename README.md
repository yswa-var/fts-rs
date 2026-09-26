quest completed:
- Dhan auth
- quote feacther
- historical loader
- Live web socket
- jetStream
quest remaning:
- Bar Aggregator
- Click House connector 
- Analytics api


redis
```

docker rm redis
docker run -d --name redis -p 6379:6379 redis:latest
 docker exec redis redis-cli
 docker exec -it redis redis-cli FLUSHALL
```
Main feed producer

This is your Dhan → parser → Redis process:
```
cargo run 
```
In another terminal:
```
cargo run --bin tick_subscriber
```

clickhouse config 
```
docker run -d \
  --name clickhouse \
  --restart unless-stopped \
  -p 8123:8123 \
  -p 9000:9000 \
  -e CLICKHOUSE_DB=quant \
  -e CLICKHOUSE_USER=quant \
  -e CLICKHOUSE_PASSWORD=quantpass \
  -e CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1 \
  -v clickhouse_data:/var/lib/clickhouse \
  clickhouse/clickhouse-server:latest
```

## Historical cache worker

Start ClickHouse, ensure `master.csv` and the Dhan credential environment variables are present, then run:

```bash
cargo run --bin historical_worker
```

The worker checks ClickHouse and creates `bar_1m` and `bar_daily` on startup. It listens on `127.0.0.1:3001` by default; set `HISTORICAL_WORKER_ADDR` to override it.

Backfill one or more security IDs with either `1m` or `daily` candles:

```bash
curl --request POST http://127.0.0.1:3001/historical/backfill \
  --header 'Content-Type: application/json' \
  --data '{"security_ids":["1333","11536"],"timeframe":"daily"}'
```
