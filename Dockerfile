FROM rust:1.84 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y sqlite3 ca-certificates libssl3 nginx gettext-base
COPY --from=builder /app/target/release/tg_spending_tracker .
COPY --from=builder /app/src/migrations ./migrations
COPY nginx/nginx.conf /etc/nginx/nginx.conf
COPY scripts/start.sh .
RUN chmod +x start.sh
RUN mkdir -p /app/data && chmod 777 /app/data
VOLUME /app/data
CMD ["./start.sh"]
