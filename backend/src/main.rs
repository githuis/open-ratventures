mod static_string_loader;

use axum::{ Extension, Router, response::Json, routing::{get, post}, };
use color_eyre::Result;
use ratback::db::DbConnection;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use ratback::data::User;
use static_string_loader::{load_dialogues, load_enemies};

#[tokio::main]
async fn main() -> Result<()> {
    let db: DbConnection = DbConnection::new().await.unwrap();
    let dialogues = load_dialogues();
    let enemies = load_enemies();

    let app = Router::new()
        .route("/api/hello-world", get(hello_world))
        .route("/api/register", post(register))
        .nest("/api", ratback::quest::routes())
        .nest("/api", ratback::users::routes())
        .layer(Extension(enemies))
        .layer(Extension(dialogues))
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
    Extension(db): Extension<DbConnection>,
    username: String,
) -> Json<User> {
    let registered_user = db.register_user(username).await.unwrap();
    let result = Json(registered_user);

    result
}

