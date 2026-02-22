use axum::{
    Router,
    extract::State,
    handler::Handler,
    response::Json,
    routing::{get, post},
};
use color_eyre::Result;
use ratback::{Character, Stats, Unit, User};
use serde_json::{Value, json};
use std::net::SocketAddr;
use tokio::net::TcpListener;

mod data;
mod quest;
mod quest_data;

#[derive(Clone, Debug, Default)]
struct ServerState {
    //users: [User; 100],
    user: User,
}

#[tokio::main]
async fn main() -> Result<()> {

    let app = Router::new() //with_state(ServerState::default())
        .route("/api/hello-world", get(hello_world))
        .route("/api/register", post(register))
        .route("/api/character", post(create_character))
        .nest("/api", quest::routes())
        .with_state(ServerState::default());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn hello_world() -> &'static str {
    "Hello World"
}

async fn register(username: String) -> Json<User> {
    let usr = User {
        username: username,
        ..Default::default()
    };

    println!("Registered user: {}", usr.username);

    Json(usr)
}

async fn create_character() -> Json<Character> {
    let chr = Character::new();

    Json(chr)
}
