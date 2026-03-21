use axum::{
    Extension, Json, Router,
    extract::Path,
    http::StatusCode,
    routing::{get, post, put},
};
use rand::Rng;
use std::{collections::HashMap, sync::Arc};

use crate::data::{CharacterWrapper, MAX_ENCOUNTER_LENGTH, Unit};
use crate::db::DbConnection;
use crate::quest_data::{Combat, CompleteQuestRequest, Dialogue, Encounter, JoinQuestRequest, Quest, QuestSummary, UpdateEncountersRequest};

type DialogueMap = Arc<HashMap<String, Dialogue>>;

pub fn routes() -> Router {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/quest/open", get(open_quests))
        .route("/quest/join", post(join_quest))
        .route("/quest/complete", post(complete_quest))
        .route("/quest/{id}", get(get_quest))
        .route("/quest/{id}/encounters", put(update_encounters))
        .route("/quest/{id}/members", get(quest_members))
        .route("/dialogue/{id}", get(get_dialogue))
}

async fn init_quest(
    Extension(db): Extension<DbConnection>,
    Extension(dialogues): Extension<DialogueMap>,
    Json(user_id): Json<i32>,
) -> Json<Quest> {
    if let Some(existing) = db.get_quest_for_user(user_id).await {
        return Json(existing);
    }
    let ids: Vec<String> = dialogues.keys().cloned().collect();
    let quest = db.new_quest(make_encounters(&ids), user_id).await.unwrap();
    Json(quest)
}

async fn open_quests(
    Extension(db): Extension<DbConnection>,
) -> Result<Json<Vec<QuestSummary>>, StatusCode> {
    db.list_open_quests().await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn join_quest(
    Extension(db): Extension<DbConnection>,
    Json(req): Json<JoinQuestRequest>,
) -> Result<Json<Quest>, StatusCode> {
    db.join_quest(req.quest_id, req.user_id).await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_quest(
    Extension(db): Extension<DbConnection>,
    Path(id): Path<i32>,
) -> Result<Json<Quest>, StatusCode> {
    db.get_quest_by_id(id).await.map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn update_encounters(
    Extension(db): Extension<DbConnection>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateEncountersRequest>,
) -> Result<StatusCode, StatusCode> {
    db.update_quest_encounters(id, req.current_encounter, req.current_node_id, req.encounters)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn quest_members(
    Extension(db): Extension<DbConnection>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<CharacterWrapper>>, StatusCode> {
    db.get_quest_members(id).await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn complete_quest(
    Extension(db): Extension<DbConnection>,
    Json(req): Json<CompleteQuestRequest>,
) -> Result<Json<CharacterWrapper>, StatusCode> {
    db.complete_quest(req.quest_id, req.user_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_dialogue(
    Extension(dialogues): Extension<DialogueMap>,
    Path(id): Path<String>,
) -> Result<Json<Dialogue>, StatusCode> {
    dialogues.get(&id).cloned().map(Json).ok_or(StatusCode::NOT_FOUND)
}

fn make_encounters(dialogue_ids: &[String]) -> Vec<Encounter> {
    let mut rng = rand::thread_rng();
    (0..MAX_ENCOUNTER_LENGTH).map(|i| {
        if i % 2 == 0 {
            let count = rng.gen_range(1..=5);
            let hp_each = (30 / count).max(1);
            let monsters = (0..count)
                .map(|_| Unit {
                    health: hp_each as i32,
                    max_health: hp_each as i32,
                    energy: 10,
                    max_energy: 10,
                    ..Default::default()
                })
                .collect();
            Encounter::CombatEncounter(Combat { monsters, turn: 0 })
        } else if !dialogue_ids.is_empty() {
            let idx = rng.gen_range(0..dialogue_ids.len());
            Encounter::NpcEncounter(dialogue_ids[idx].clone())
        } else {
            Encounter::EmptyEncounter
        }
    }).collect()
}
