# Dhan Market Data Pipeline

## Executive summary

This pipeline turns Dhan market data into two products:

1. A bounded, replayable stream of normalized live ticks for short-horizon consumers.
2. Enriched one-minute bars in ClickHouse for research, analytics, and downstream serving.

Instrument master data and historical daily candles support discovery and backfill. They are intentionally separate from the low-latency live path.

The design favors a small, local operational footprint and rapid iteration over multi-region availability or permanent raw-tick retention. Redis is a transport and recovery buffer; ClickHouse is the analytical destination for completed bars.

## Architecture at a glance

```text
                       Control plane
              ┌─────────────────────────────┐
              │ Dhan authentication          │
              │ Instrument master             │
              └──────────────┬──────────────┘
                             │ credentials + instrument universe
                             ▼
                        Live data plane

Dhan WebSocket ──> Feed gateway ──> Redis Stream (`ticks`) ──> Bar service ──> ClickHouse
                       │                    │                    │               │
                       │                    │                    │               └─ 1-minute live bars
                       │                    │                    └─ event-time aggregation
                       │                    └─ bounded replay / consumer hand-off
                       └─ validation, normalization, reconnect

Dhan Historical API ──> Daily-candle snapshot files
```

## Data products and ownership

| Product | Primary use | Current store | Lifecycle |
| --- | --- | --- | --- |
| Instrument master | Resolve tradable instruments and Dhan identifiers | Dhan scrip master; local CSV cache | Replaced on refresh; versioning is still required |
| Historical daily candles | Research and backfill input | JSON snapshots today | Managed as files |
| Live normalized ticks | Near-real-time processing and short replay | Redis Stream | Bounded by `TICKS_MAXLEN` |
| Live one-minute bars | Analytics and downstream query workloads | ClickHouse | Long-term analytical storage |

The ClickHouse bar schema can represent both historical and live bars. At present, live-derived bars are written there; historical daily snapshots are not yet loaded into ClickHouse.

## Core design decisions

### Use a single normalized tick stream

All subscribed instruments are written to one Redis Stream, `ticks`, rather than one stream per security. This keeps consumer topology simple, establishes one ingestion order, and makes it easy to add consumers without managing a large number of Redis keys. Instrument identity remains part of every event, so consumers can partition or filter when needed.

The stream is capped approximately at a configurable length. This is a deliberate product decision: it provides a recovery window for live consumers without turning Redis into an unbounded raw-market-data archive. The correct value is driven by peak tick rate and the required recovery period.

### Treat event time as canonical

One-minute bars are assigned from Dhan's last-trade timestamp, not the time the application received the message. This keeps the bar aligned with market activity despite transport jitter or reconnects. Bars include OHLCV plus close-of-minute order-flow, top-of-book, depth, and open-interest features so research consumers do not need to reconstruct those signals from raw ticks.

### Separate ingest, aggregation, and storage

The feed gateway only validates, normalizes, and appends ticks. It does not perform database writes or analytics. The bar service owns event-time aggregation, and ClickHouse is the long-term analytical destination. This separation allows additional consumers—alerts, execution tooling, or alternate aggregations—to be added without changing the feed connection.

### Keep the control plane explicit

Authentication and the instrument master determine what the pipeline is permitted to request. The running live feed currently selects its instrument universe from the master-data tag `FNO`. Refreshing or versioning that universe is an operational control, not an implicit property of the feed process.

### Preserve a simple local-first deployment model

The current architecture requires the Rust services, Redis, and ClickHouse. It does not depend on a separate streaming platform. NATS JetStream or another durable event bus remains a future option if the product needs independent retention, cross-host fan-out, or stronger delivery guarantees than Redis can provide.

## Processing behavior

### Live ticks

The feed gateway establishes an authenticated Dhan WebSocket session, subscribes to the selected instruments, validates full-depth messages, and reconnects with capped exponential backoff after connection failures. It responds to WebSocket keep-alives and re-sends the subscription after reconnecting.

Each accepted message becomes a normalized tick containing the instrument key, trade fields, aggregate quantities, open interest, and five levels of market depth. Malformed messages are rejected rather than propagated.

### One-minute bars

The bar service reads the `ticks` stream through a Redis consumer group. It maintains one active minute window per instrument and emits the previous bar when it receives the first tick for a later minute. Cumulative volume is converted to incremental volume; if a cumulative counter resets, the reported last-trade quantity is used as a fallback.

The service intentionally ignores out-of-order ticks from already-finalized minutes. This produces stable bars without retroactive correction, which is appropriate for the current live analytics product. It also means a bar for an inactive instrument is not emitted until a later tick arrives; there is no clock-driven idle-window close today.

### Analytical storage

Completed live bars are queued and inserted into ClickHouse in batches. Batching is configurable by size and time interval; transient insert failures are retried with capped exponential backoff. The schema distinguishes historical from live rows and permits live-only microstructure fields to be absent in historical data.

## Delivery and recovery posture

The pipeline is designed for operationally useful recovery, not end-to-end exactly-once delivery.

| Boundary | Current posture | Implication |
| --- | --- | --- |
| Dhan to feed gateway | Reconnect and re-subscribe | Data during an upstream or credential outage may be missed. |
| Feed gateway to Redis | Append to a bounded stream | Recent data can be replayed while retained; older ticks are deliberately discarded. Redis persistence must be configured for the desired crash durability. |
| Redis to bar service | Consumer-group processing | A valid tick is acknowledged after it has been accepted by the in-process bar pipeline. A process failure can therefore lose acknowledged, not-yet-committed bars. |
| Bar service to ClickHouse | Retried batch insert | Transient database outages stall and retry the in-memory batch; a process crash before a successful insert requires reconciliation because the source event may already be acknowledged. |

This is sufficient for an early live analytics pipeline, but it is not a ledger or regulatory record. Consumers requiring authoritative tick history need a durable raw-event store and explicit replay/idempotency controls.

## Operational decisions still required

Before treating the pipeline as production-critical, the following choices need owners and service-level objectives:

- **Retention:** define the Redis replay window from peak throughput and recovery objectives; decide whether raw ticks also require long-term archival.
- **Availability:** refresh credentials before expiry and define alerting for feed disconnects, stalled consumers, and ClickHouse retry loops.
- **Bar completeness:** choose an idle-window close policy and whether late data may revise previously emitted bars.
- **Scaling:** preserve per-instrument ordering when adding bar workers. A shared consumer group alone can distribute ticks for the same instrument across workers and fragment its bar state.
- **Delivery semantics:** decide whether acknowledged-before-ClickHouse-commit loss is acceptable. If not, use durable hand-off, transactional/idempotent writes, and replay.
- **Data governance:** version the instrument master and tick/bar schemas, retain provenance, and define how corrections are handled.

## Product boundary

This pipeline is an ingestion and market-data preparation layer. It does not make trading decisions, place orders, or expose an analytics API. Its purpose is to provide a dependable foundation on which those products can be built.
