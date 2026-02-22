use axum::{Json, Router, extract::State, routing::post};
use serde_json::json;

use crate::{ServerState, quest_data::Quest,};


pub fn routes() -> Router<ServerState> {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/combat", post(init_combat))
}

async fn init_quest(State(state): State<ServerState>) -> Json<Quest> {
    Json(Quest::default())

}

async fn init_combat(State(state): State<ServerState>) -> Json<Quest> {
    Json(Quest::default())
}
