FROM rust:1.84 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y sqlite3 ca-certificates libssl3
COPY --from=builder /app/target/release/tg_spending_tracker .
COPY --from=builder /app/src/migrations ./migrations
RUN mkdir -p /app/data && chmod 777 /app/data
CMD ["./tg_spending_tracker"]
