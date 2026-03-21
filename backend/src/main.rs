use axum::{ Extension, Router, response::Json, routing::{get, post}, };
use color_eyre::Result;
use ratback::{ data::{CharacterWrapper, ServerState, SharedState, Unit}, db::DbConnection, };
use sqlx::{Connection, SqliteConnection, sqlite::SqlitePoolOptions};
use std::{ net::SocketAddr, sync::{Arc, RwLock}, };
use tokio::net::TcpListener;
use ratback::data::{Character, User};

#[tokio::main]
async fn main() -> Result<()> {
    let state = ServerState {
        users: vec![],
        characters: vec![],
        quests: vec![],
    };

    // Extensions
    let shared_state: SharedState = SharedState::new(RwLock::new(state));
    let db: DbConnection = DbConnection::new().await.unwrap();

    let app = Router::new() //with_state(ServerState::default())
        .route("/api/hello-world", get(hello_world))
        .route("/api/register", post(register))
        .route("/api/character", post(create_character))
        .nest("/api", ratback::quest::routes())
        .layer(Extension(shared_state))
        .layer(Extension(db));

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
    Extension(db): Extension<DbConnection>,
    username: String,
) -> Json<User> {
    let registered_user = db.register_user(username).await.unwrap();
    let result = Json(registered_user);

    result
}

async fn create_character(
    Extension(state): Extension<SharedState>,
    Extension(db): Extension<DbConnection>,
    user_id: String,
) -> Json<CharacterWrapper> {
    
    
    Json(db.get_character(user_id).await.unwrap())
}
