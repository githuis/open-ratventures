use axum::{
    Extension, Router, extract::State, handler::Handler, response::Json, routing::{get, post}
};
use color_eyre::Result;
use ratback::data::{ServerState, SharedState};

use serde_json::{Value, json};
use std::{net::SocketAddr, sync::{Arc, RwLock}};
use tokio::net::TcpListener;

use ratback::data::{Character, Item, MAX_COMBAT_ENEMIES, MAX_ENCOUNTER_LENGTH, MAX_PARTY_SIZE, User};
use ratback::quest_data::Quest;


#[tokio::main]
async fn main() -> Result<()> {

    let state = ServerState {
            users: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            characters: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None],
            quests: [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None]
        };

    let x: SharedState = SharedState::new(RwLock::new(state));

    let app = Router::new() //with_state(ServerState::default())
        .route("/api/hello-world", get(hello_world))
        .route("/api/register", post(register))
        .route("/api/character", post(create_character))
        .nest("/api", ratback::quest::routes())
        .layer(Extension(x))
        ;

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

async fn hello_world() -> &'static str {
    "Hello World"
}

async fn register(Extension(state): Extension<SharedState>, username: String) -> Json<User> {
    let usr = User {
        username: username,
        ..Default::default()
    };

    println!("Registered user: {}", usr.clone().username);
    let result = Json(usr.clone());

    for x in state.write().unwrap().users.iter_mut() {
        match x {
            None => {
                *x = Some(usr);
                break;
            }
            _ => {}
            
        }
    }
    

    result
}

async fn create_character(Extension(state): Extension<SharedState>) -> Json<Character> {
    let chr = Character::new();

    for x in state.write().unwrap().characters.iter_mut() {
        match x {
             None => {
                *x = Some(chr);
                break;
            }
            _ => {}
            
        }
    }

    Json(chr)
}
