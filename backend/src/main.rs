mod static_string_loader;

use axum::{ Extension, Router, response::Json, routing::{get, post}, };
use tower_http::cors::CorsLayer;
use color_eyre::Result;
use ratback::db::DbConnection;
use tokio::net::TcpListener;
use ratback::data::User;
use static_string_loader::{load_dialogues, load_enemies, load_missions};

#[tokio::main]
async fn main() -> Result<()> {
    let db: DbConnection = DbConnection::new().await.unwrap();
    let dialogues = load_dialogues();
    let enemies = load_enemies();
    let missions = load_missions();

    let app = Router::new()
        .route("/api/hello-world", get(hello_world))
        .route("/api/version", get(version))
        .route("/api/register", post(register))
        .nest("/api", ratback::party::routes())
        .nest("/api", ratback::quest::routes())
        .nest("/api", ratback::users::routes())
        .layer(Extension(missions))
        .layer(Extension(enemies))
        .layer(Extension(dialogues))
        .layer(Extension(db))
        .layer(CorsLayer::permissive());

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(3000);
    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn hello_world() -> &'static str {
    "Hello World"
}

async fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

async fn register(
    Extension(db): Extension<DbConnection>,
    username: String,
) -> Json<User> {
    let registered_user = db.register_user(username).await.unwrap();
    let result = Json(registered_user);

    result
}

