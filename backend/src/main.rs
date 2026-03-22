use axum::{ Extension, Router, response::Json, routing::{get, post}, };
use color_eyre::Result;
use ratback::db::DbConnection;
use std::{ collections::HashMap, net::SocketAddr, sync::Arc };
use tokio::net::TcpListener;
use ratback::data::User;
use ratback::quest_data::{Dialogue, Monster};

fn load_enemies() -> Arc<Vec<Monster>> {
    let candidates = ["backend/data/enemies", "data/enemies"];
    let dir = candidates.iter().map(std::path::Path::new).find(|p| p.exists());

    let mut monsters = Vec::new();
    if let Some(dir) = dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                match std::fs::read_to_string(entry.path())
                    .ok()
                    .and_then(|s| serde_json::from_str::<Monster>(&s).ok())
                {
                    Some(m) => monsters.push(m),
                    None => eprintln!("Failed to parse enemy '{}'", entry.path().display()),
                }
            }
        }
    }
    println!("Loaded {} enemy templates", monsters.len());
    Arc::new(monsters)
}

fn load_dialogues() -> Arc<HashMap<String, Dialogue>> {
    let candidates = ["backend/data/dialogues", "data/dialogues"];
    let dir = candidates.iter().map(std::path::Path::new).find(|p| p.exists());

    let mut map = HashMap::new();
    if let Some(dir) = dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    let id = path.file_stem().unwrap().to_string_lossy().to_string();
                    match std::fs::read_to_string(&path) {
                        Err(e) => println!("Failed to read {id}: {e}"),
                        Ok(content) => match serde_json::from_str::<Dialogue>(&content) {
                            Ok(dialogue) => { map.insert(id, dialogue); }
                            Err(e) => println!("Failed to parse dialogue '{id}': {e}"),
                        },
                    }
                }
            }
        }
    }
    println!("Loaded {} dialogues: {:?}", map.len(), map.keys().collect::<Vec<_>>());
    Arc::new(map)
}

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

