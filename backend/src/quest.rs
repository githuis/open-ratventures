use axum::{Extension, Json, Router, extract::State, routing::post};
use serde_json::json;

use crate::data::{Character, ServerState, SharedState};
use crate::quest_data::{Encounter, Quest};

pub fn routes() -> Router {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/combat", post(init_combat))
}

async fn init_quest(Extension(state): Extension<SharedState>) -> Json<Quest> {
    let mut quest = Quest::default();

    //jquest.encounters.push(make_encounter());
    //quest.encounters.push(make_encounter());

    Json(quest)
}

async fn init_combat(Extension(state): Extension<SharedState>) -> Json<Quest> {
    Json(Quest::default())
}

fn make_encounter() -> Encounter {
    Encounter::NpcEncounter(crate::quest_data::EncounterReward::ExperienceReward(32))
}
