# --- Build stage ---
FROM rust:1.87-slim-bookworm AS builder

RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs \
    && cargo build --release \
    && rm -rf src

# Build the real binary
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs src/lib.rs && cargo build --release

# --- Runtime stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libsqlite3-0 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/ratback-bin ./ratback-bin
COPY data ./data

ENV HOST=0.0.0.0
ENV PORT=3000
ENV DATABASE_URL=sqlite:///app/db/data.db

EXPOSE 3000

# Mount /app/db to persist the database across container restarts
VOLUME ["/app/db"]

CMD ["./ratback-bin"]
