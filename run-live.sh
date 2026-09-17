#!/usr/bin/env bash

set -e

# Remove the previous Redis container if it exists.
docker rm redis

docker run -d --name redis -p 6379:6379 redis:latest

# docker run -d --name clickhouse \
#   -p 8123:8123 \
#   -p 9000:9000 \
#   -e CLICKHOUSE_DB=quant \
#   -e CLICKHOUSE_USER=quant \
#   -e CLICKHOUSE_PASSWORD=quantpass \
#   clickhouse/clickhouse-server

docker start clickhouse

until docker exec redis redis-cli ping >/dev/null 2>&1; do
  sleep 1
done

until curl -fsS http://127.0.0.1:8123/ping >/dev/null 2>&1; do
  sleep 1
done

cargo run &
FEED_PID=$!

cleanup() {
  kill "$FEED_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

sleep 2
cargo run --bin bar_aggregator
