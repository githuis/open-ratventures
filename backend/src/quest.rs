use axum::{
    Extension, Json, Router,
    extract::Path,
    http::StatusCode,
    routing::{get, post, put},
};
use rand::Rng;
use std::{collections::HashMap, sync::Arc};

use crate::data::{CharacterWrapper, MAX_ENCOUNTER_LENGTH};
use crate::db::DbConnection;
use crate::quest_data::{Combat, CompleteQuestRequest, Dialogue, Encounter, JoinQuestRequest, Monster, Quest, QuestSummary, UpdateEncountersRequest};

#[derive(serde::Deserialize)]
struct NewQuestRequest {
    user_id: i32,
    area: String,
}

type DialogueMap = Arc<HashMap<String, Dialogue>>;
type EnemyList = Arc<Vec<Monster>>;

pub fn routes() -> Router {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/quest/open", get(open_quests))
        .route("/quest/join", post(join_quest))
        .route("/quest/complete", post(complete_quest))
        .route("/quest/{id}", get(get_quest))
        .route("/quest/{id}/encounters", put(update_encounters))
        .route("/quest/{id}/members", get(quest_members))
        .route("/quest/active/{user_id}", get(get_active_quest_for_user))
        .route("/dialogue/{id}", get(get_dialogue))
}

async fn init_quest(
    Extension(db): Extension<DbConnection>,
    Extension(dialogues): Extension<DialogueMap>,
    Extension(enemies): Extension<EnemyList>,
    Json(req): Json<NewQuestRequest>,
) -> Json<Quest> {
    if let Some(existing) = db.get_quest_for_user(req.user_id).await {
        return Json(existing);
    }
    let party_renown = db.get_character(req.user_id.to_string()).await
        .map(|c| c.character.renown)
        .unwrap_or(0);
    let quest = db.new_quest(make_encounters(&dialogues, &enemies, party_renown, &req.area), req.user_id).await.unwrap();
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
    db.complete_quest(req.quest_id, req.user_id, 5, 1).await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_active_quest_for_user(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
) -> Result<Json<Quest>, StatusCode> {
    db.get_quest_for_user(user_id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn get_dialogue(
    Extension(dialogues): Extension<DialogueMap>,
    Path(id): Path<String>,
) -> Result<Json<Dialogue>, StatusCode> {
    dialogues.get(&id).cloned().map(Json).ok_or(StatusCode::NOT_FOUND)
}

fn make_encounters(dialogues: &HashMap<String, Dialogue>, enemies: &[Monster], party_renown: u32, area: &str) -> Vec<Encounter> {
    let eligible_enemies: Vec<&Monster> = enemies.iter()
        .filter(|e| e.required_renown <= party_renown)
        .filter(|e| e.areas.is_empty() || e.areas.iter().any(|a| a == area))
        .collect();
    let eligible_dialogues: Vec<&str> = dialogues.values()
        .filter(|d| d.required_renown <= party_renown)
        .filter(|d| d.areas.is_empty() || d.areas.iter().any(|a| a == area))
        .map(|d| d.id.as_str())
        .collect();

    let mut rng = rand::thread_rng();
    (0..MAX_ENCOUNTER_LENGTH).map(|i| {
        if i % 2 == 0 && !eligible_enemies.is_empty() {
            let count = rng.gen_range(1..=3usize);
            let monsters = (0..count)
                .map(|_| {
                    let t = eligible_enemies[rng.gen_range(0..eligible_enemies.len())];
                    Monster {
                        unit: t.unit,
                        name: t.name.clone(),
                        attack: t.attack,
                        items: t.items.clone(),
                        required_renown: t.required_renown,
                        areas: t.areas.clone(),
                    }
                })
                .collect();
            Encounter::CombatEncounter(Combat { monsters, turn: 0 })
        } else if !eligible_dialogues.is_empty() {
            let idx = rng.gen_range(0..eligible_dialogues.len());
            Encounter::NpcEncounter(eligible_dialogues[idx].to_string())
        } else {
            Encounter::EmptyEncounter
        }
    }).collect()
}
