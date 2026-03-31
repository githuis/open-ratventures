# --- Build stage ---
FROM rust:1.87-slim-bookworm AS builder

RUN apt-get update && apt-get install -y libsqlite3-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace Cargo files
COPY Cargo.toml Cargo.lock ./
COPY ratback-types/Cargo.toml ./ratback-types/Cargo.toml
COPY backend/Cargo.toml ./backend/Cargo.toml
COPY rat-client/Cargo.toml ./rat-client/Cargo.toml

# Cache dependencies with dummy sources
RUN mkdir -p backend/src ratback-types/src rat-client/src \
    && echo "fn main() {}" > backend/src/main.rs \
    && echo "" > backend/src/lib.rs \
    && echo "" > ratback-types/src/lib.rs \
    && echo "fn main() {}" > rat-client/src/main.rs \
    && cargo build --release -p ratback \
    && rm -rf backend/src ratback-types/src rat-client/src

# Build the real binary
COPY backend/src ./backend/src
COPY backend/migrations ./backend/migrations
COPY ratback-types/src ./ratback-types/src
RUN touch backend/src/main.rs backend/src/lib.rs ratback-types/src/lib.rs \
    && cargo build --release -p ratback

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

# Mount /app/db to persist the database across container restarts
VOLUME ["/app/db"]

CMD ["./ratback-bin"]
