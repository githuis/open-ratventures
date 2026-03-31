# --- Build stage ---
FROM rust:1.87-slim-bookworm AS builder

RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .

RUN cargo build --release -p ratback

# --- Runtime stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libsqlite3-0 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/ratback-bin ./ratback-bin
COPY backend/data ./data

ENV HOST=0.0.0.0
ENV PORT=3000
ENV DATABASE_URL=sqlite:///app/db/data.db

EXPOSE 3000

VOLUME ["/app/db"]

CMD ["./ratback-bin"]
