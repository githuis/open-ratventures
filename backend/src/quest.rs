
use axum::{Extension, Json, Router, extract::State, routing::post};
use serde_json::json;

use crate::data::{Character, SharedState, ServerState};
use crate::quest_data::Quest;


pub fn routes() -> Router {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/combat", post(init_combat))
}

async fn init_quest(Extension(state): Extension<SharedState>) -> Json<Quest> {
    let mut quest = Quest::default();

    let mut j = 0;
    for c in state.write().unwrap().characters.iter() {
        match c {
            Some(ch) => {
                quest.members[j] = *ch;
                j += 1;
            }
            None => break,
        }

        if j >= 3 {
            break;
        }
    }

    Json(quest)
}

async fn init_combat(Extension(state): Extension<SharedState>) -> Json<Quest> {
    Json(Quest::default())
}
