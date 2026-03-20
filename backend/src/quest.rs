use axum::{Extension, Json, Router, extract::State, routing::post};
use serde_json::json;

use crate::data::{Character, ServerState, SharedState};
use crate::db::{self, DbConnection};
use crate::quest_data::{Encounter, Quest};

pub fn routes() -> Router {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/combat", post(init_combat))
}

async fn init_quest(Extension(db): Extension<DbConnection>, Json(user_id): Json<i32>) -> Json<Quest> {
    if let Some(existing) = db.get_quest_for_user(user_id).await {
        return Json(existing);
    }

    let quest = db.new_quest().await.unwrap();
    Json(quest)
}

async fn init_combat(Extension(state): Extension<SharedState>) -> Json<Quest> {
    Json(Quest::default())
}

fn make_encounter() -> Encounter {
    Encounter::NpcEncounter(crate::quest_data::EncounterReward::ExperienceReward(32))
}
