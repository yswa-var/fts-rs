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
