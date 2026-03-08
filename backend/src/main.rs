use axum::{
    Extension, Router,
    extract::State,
    handler::Handler,
    response::Json,
    routing::{get, post},
};
use color_eyre::Result;
use ratback::data::{ServerState, SharedState};
use sqlx::{sqlite::SqlitePoolOptions};

use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use tokio::net::TcpListener;

use ratback::data::{
    Character, Item, MAX_COMBAT_ENEMIES, MAX_ENCOUNTER_LENGTH, MAX_PARTY_SIZE, User,
};
use ratback::quest_data::Quest;

#[tokio::main]
async fn main() -> Result<()> {
    let state = ServerState {
        users: vec![],
        characters: vec![],
        quests: vec![],
    };

    let shared_state: SharedState = SharedState::new(RwLock::new(state));
    let conn = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://data.db");

    let app = Router::new() //with_state(ServerState::default())
        .route("/api/hello-world", get(hello_world))
        .route("/api/register", post(register))
        .route("/api/character", post(create_character))
        .nest("/api", ratback::quest::routes())
        .layer(Extension(shared_state))
        
        //.layer(Extension(dbconn))
        ;

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn hello_world() -> &'static str {
    "Hello World"
}

async fn register(
    Extension(state): Extension<SharedState>,
    //Extension(conn): Extension<SqliteConnection>,
    username: String,
) -> Json<User> {
    let usr = User {
        username: username,
        ..Default::default()
    };

    println!("Registered user: {}", usr.clone().username);
    let result = Json(usr.clone());
    //conn.execute("INSERT INTO user (username, character)", params);

    state.write().unwrap().users.push(Some(usr));

    result
}

async fn create_character(Extension(state): Extension<SharedState>) -> Json<Character> {
    let chr = Character::default();

    state.write().unwrap().characters.push(Some(chr));

    Json(chr)
}
