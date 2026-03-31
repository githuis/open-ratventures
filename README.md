# Ratventures

Ratventures with friends, in the terminal!

**Play now at [ratventure.online](https://ratventure.online)**

Go on quests with up to 2 friends, defeat monsters, solve riddles, and explore the depths of the sewer underworld. Built with Rust — a ratatui TUI client talking to an axum/SQLite backend.

## Structure

```
open-ratventures/
├── backend/        # axum HTTP server + SQLite database (ratback crate)
├── rat-client/     # ratatui terminal UI client
└── docker/         # Dockerfile for backend
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [sqlx-cli](https://github.com/launchbadge/sqlx) *(optional, only needed for database resets)*

```bash
cargo install sqlx-cli --no-default-features --features sqlite
```

## Running locally

**1. Start the backend**

```bash
cd backend
cargo run
```

The server starts on `http://localhost:3000` by default. Set `HOST` or `PORT` env vars to override.

**2. Start the client** (in a separate terminal)

```bash
cd rat-client
cargo run
```

### VSCode

A compound task is included. Press the configured hotkey (or **Terminal → Run Task → Run rats**) to launch both in separate terminals at once.

## Database

The backend uses SQLite (`backend/data.db`). Migrations run automatically on startup via sqlx.

If you need a clean slate:

```bash
cd backend
sqlx database reset
```

After resetting or adding new migrations, touch `src/db.rs` before running to force the migration macro to recompile:

```bash
touch src/db.rs && cargo run
```

## Running with Docker

The backend can be run via Docker Compose. The database is persisted in a named volume.

```bash
docker compose up
```

Then run the client locally as above, pointing at the Docker backend (default port 3000).

## Gameplay

- Register a username and create a character
- Form or join a party (up to 3 players)
- Go on a quest through procedurally assigned encounters across four areas:
  - **Sewers** — the upper tunnels, relatively tame
  - **Sewer Depths** — darker, more dangerous
  - **Fungal Warrens** — bioluminescent caverns, spore-touched inhabitants
  - **Abyss** — the void-touched deep, where things get strange
- Encounters include combat, NPC dialogues, merchants, and environmental events
- Manage your inventory, buy items from the tavern shop, and revive fallen party members
