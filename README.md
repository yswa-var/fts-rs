quest completed:
- Dhan auth
- quote feacther
- historical loader

quest remaning:
- Live web socket
- NATS jetStream
- Bar Aggregator
- Click House connector 
- Analytics api


redis
```
docker run --name redis -p 6379:6379 redis:latest
```
Main feed producer

This is your Dhan → parser → Redis process:

cargo run --bin fts-rs
subscriber

In another terminal:

cargo run --bin tick_subscriber
