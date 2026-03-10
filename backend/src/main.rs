use axum::{
    Extension, Router,
    extract::State,
    handler::Handler,
    response::Json,
    routing::{get, post},
};
use color_eyre::Result;
use ratback::data::{ServerState, SharedState};
use sqlx::{Connection, SqliteConnection, sqlite::SqlitePoolOptions};

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

    let shared_db = Arc::new(RwLock::new(conn));

    let app = Router::new() //with_state(ServerState::default())
        .route("/api/hello-world", get(hello_world))
        .route("/api/register", post(register))
        .route("/api/character", post(create_character))
        .nest("/api", ratback::quest::routes())
        .layer(Extension(shared_state))
        //.layer(Extension(shared_db))
        
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

async fn register( Extension(state): Extension<SharedState>, username: String,) -> Json<User> {
    let mut try_conn = SqliteConnection::connect("sqlite://data.db").await.unwrap();


    let mut stream  = sqlx::query_as::<_, User>("SELECT * FROM users where username = $1")
        .bind(username.clone())
        .fetch_one(&mut try_conn).await;

    let new_user = match stream {
        Ok(u) => {
            println!("Fetched user from database {}", u.username);

            u
        },
        Err(e) => {
            println!("Couldn't find user with name {}, registered new user. Msg: {}", username.clone(), e.to_string());

            User {
            username: username,
            ..Default::default()
        }
    },
    };

    //try_conn.execute(sqlx::query("insert into user(username) values ($1)")).bind(new_user.clone().username);

    println!("Registered user: {}", new_user.clone().username);
    let result = Json(new_user.clone());
    //conn.execute("INSERT INTO user (username, character)", params);

    result
}

async fn create_character(Extension(state): Extension<SharedState>) -> Json<Character> {
    let chr = Character::default();

    state.write().unwrap().characters.push(Some(chr));

    Json(chr)
}
