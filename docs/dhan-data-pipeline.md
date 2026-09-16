# Dhan Data Pipeline

## Purpose

This project is a Rust-based ingestion layer for Dhan market data. It currently supports:

- Historical daily candle downloads through Dhan's REST API.
- Live full-market-data ticks through Dhan's WebSocket feed.
- Durable-enough local fan-out of live ticks through Redis Streams.
- Downloading Dhan's scrip master file for instrument metadata and security-ID lookup.

The intended downstream pipeline is:

```text
Dhan APIs
   ├── Historical REST API ──> hdata.rs ──> data/<symbol>_daily.json
   ├── Scrip master CSV ────> master.rs ──> master.csv
   └── Live WebSocket ──────> feed/ ──────> Redis Streams ──> consumers
                                                        └─> future aggregators,
                                                            storage, analytics
```

## Startup and authentication

`main.rs` is the current application entry point. It obtains a Dhan access token and client ID through `auth.rs`, then starts the live feed with a hard-coded subscription.

Authentication uses `CLIENT_ID`, `TOTP_KEY`, and `DPIN` from the environment or `.env`. A successful authentication response is cached in `auth.json` and reused until its Dhan-provided expiry time. The access token is required by both the historical REST request and the WebSocket connection.

`master.rs` is a separate synchronous utility. It downloads Dhan's detailed scrip master CSV from the Dhan data endpoint and overwrites `master.csv`. This file is the reference source for resolving exchange segments, security IDs, instruments, and other instrument metadata before constructing subscriptions or historical requests.

## Historical data path: `hdata.rs`

`hdata.rs` provides the batch/backfill path for daily candles.

### Input

Each requested symbol is represented by a `Symbol` containing:

- A local output name.
- Dhan `security_id`.
- Dhan `exchange_segment`.
- Dhan `instrument` type.

`fetch_symbols` currently chooses a rolling six-month date range ending on the current UTC date. It creates the `data/` directory and processes symbols sequentially, waiting 500 ms between requests.

### Dhan request

`get_historical` posts to Dhan's `/v2/charts/historical` endpoint. The request includes the security ID, exchange segment, instrument, date range, and chart options. The response is an array-oriented JSON object: each OHLCV field is a separate array indexed by timestamp.

### Normalization and output

The response is converted into a vector of `Candle` records. The effective record count is the shortest of the required arrays, which prevents out-of-bounds access when Dhan returns uneven data. Missing open-interest values default to zero.

Each symbol is serialized as pretty-printed JSON at:

```text
data/<symbol.name>_daily.json
```

Timestamps are retained as integer epoch values. Prices remain `f64`; volume and open interest are converted to integer values.

### Operational characteristics

- Requests fail on non-success HTTP status codes.
- A failed symbol currently stops the batch rather than being retried or skipped.
- Files are replaced on each successful fetch.
- The current implementation is a snapshot loader, not an incremental candle store.

## Live market-data path: `feed/`

The `feed` module is responsible for receiving, decoding, and publishing Dhan's binary full-tick messages.

### Subscription model

`feed::models` defines `Instrument` and `Subscription`, which serialize to Dhan's expected JSON subscription shape:

- `RequestCode` selects the feed request type.
- `InstrumentCount` declares the number of instruments.
- `InstrumentList` contains exchange segment and security ID pairs.

The current example subscription in `main.rs` requests four NSE equity instruments using request code `21`.

### Connection and reconnect: `feed/client.rs`

`feed::run` builds the Dhan WebSocket URL using the access token and client ID, creates a Redis publisher, and enters a reconnecting receive loop.

Connection setup explicitly:

1. Resolves the Dhan hostname and selects an IPv4 address.
2. Opens a TCP connection with a five-second timeout.
3. Enables TCP no-delay.
4. Establishes the TLS WebSocket with a five-second timeout.
5. Sends the serialized subscription.
6. Reads messages until the stream closes or errors.

Binary messages are treated as full packets and sent to the parser. Ping frames receive a matching Pong. Close frames and transport errors return control to the reconnect loop.

`feed/reconnect.rs` supplies exponential backoff: 2, 4, 8, 16, and 30-second delays, capped at 30 seconds. A successful connection resets the attempt counter.

### Binary decoding: `feed/parser.rs`

`parse_full` decodes the fixed 162-byte full-tick packet using little-endian fields. It validates the minimum packet size and requires response code `8`.

The decoded `FullPacket` contains:

- Packet metadata: response code, message length, exchange segment, and security ID.
- Last-trade data: LTP, quantity, timestamp, average trade price, and volume.
- Aggregate buy/sell quantities.
- Open interest and its daily high/low.
- Day OHLC values.
- Five levels of bid/ask market depth.

`parse_ticker` also supports the smaller 16-byte ticker format, although the active WebSocket path currently uses `parse_full` only.

### Redis publication: `feed/publisher.rs`

`RedisPublisher` connects to the local Redis instance at `redis://127.0.0.1/`. Each full tick is appended with `XADD` to a stream named:

```text
tick:<security_id>
```

The stream entry stores scalar quote fields plus flattened depth fields such as `depth_0_bid_price`, `depth_0_ask_qty`, and so on for all five levels. Redis generates the stream entry ID with `*`.

This gives each security an independent ordered event stream. `src/bin/tick_subscriber.rs` demonstrates a blocking `XREAD` consumer for `tick:4668`.

## End-to-end live flow

```text
auth.rs
  │ access token + client ID
  ▼
main.rs
  │ Subscription
  ▼
feed/client.rs ──WebSocket──> Dhan
  │ binary full packet
  ▼
feed/parser.rs
  │ FullPacket
  ▼
feed/publisher.rs
  │ XADD tick:<security_id>
  ▼
Redis Streams
  │
  └── tick_subscriber / future consumers
```

The historical and live paths share Dhan identifiers and authentication, but they do not currently converge in storage. Historical candles are files; live data is Redis stream data.

## Data ownership and boundaries

| Component | Responsibility | Output |
| --- | --- | --- |
| `master.rs` | Refresh instrument metadata | `master.csv` |
| `hdata.rs` | Fetch and normalize historical candles | `data/<symbol>_daily.json` |
| `feed/client.rs` | Maintain WebSocket session and receive frames | Parsed-message input |
| `feed/parser.rs` | Validate and decode binary frames | `FullPacket` / `TickerPacket` |
| `feed/publisher.rs` | Convert full ticks to Redis stream entries | `tick:<security_id>` |
| `feed/reconnect.rs` | Bound reconnect frequency | Retry delay |
| `tick_subscriber.rs` | Example downstream reader | Console output |

## Current limitations and next pipeline layers

The README identifies the following planned layers that are not yet implemented:

- NATS JetStream for broader event distribution.
- Bar aggregation from ticks into time-based OHLCV data.
- ClickHouse persistence for historical and live-derived data.
- Analytics API access.

Before adding those layers, the main production concerns are likely to be token refresh during long-running sessions, Redis stream retention and consumer groups, reconnect resubscription behavior, malformed-packet metrics, backpressure, and durable instrument-master versioning.

## Running the current pipeline

Start Redis locally:

```bash
docker run --name redis -p 6379:6379 redis:latest
```

Run the live producer:

```bash
cargo run --bin fts-rs
```

In another terminal, run the example stream reader:

```bash
cargo run --bin tick_subscriber
```

Historical loading and master-data refresh are currently exposed as Rust functions (`fetch_symbols` and `get_master`) and are not wired into the default `main.rs` execution path.
