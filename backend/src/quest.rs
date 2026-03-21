use axum::{
    Extension, Json, Router,
    extract::Path,
    http::StatusCode,
    routing::{get, post, put},
};
use rand::Rng;

use crate::data::{CharacterWrapper, MAX_ENCOUNTER_LENGTH, Unit};
use crate::db::DbConnection;
use crate::quest_data::{Combat, Dialogue, Encounter, Quest, QuestSummary};

pub fn routes() -> Router {
    Router::new()
        .route("/character/{user_id}", get(get_character))
        .route("/quest", post(init_quest))
        .route("/quest/open", get(open_quests))
        .route("/quest/join", post(join_quest))
        .route("/quest/complete", post(complete_quest))
        .route("/quest/{id}", get(get_quest))
        .route("/quest/{id}/encounters", put(update_encounters))
        .route("/quest/{id}/members", get(quest_members))
        .route("/dialogue/{id}", get(get_dialogue))
}

async fn get_character(
    Extension(db): Extension<DbConnection>,
    Path(user_id): Path<i32>,
) -> Result<Json<CharacterWrapper>, StatusCode> {
    db.get_character_by_user_id(user_id).await.map(Json).map_err(|_| StatusCode::NOT_FOUND)
}

async fn init_quest(
    Extension(db): Extension<DbConnection>,
    Json(user_id): Json<i32>,
) -> Json<Quest> {
    if let Some(existing) = db.get_quest_for_user(user_id).await {
        return Json(existing);
    }
    let quest = db.new_quest(make_encounters(), user_id).await.unwrap();
    Json(quest)
}

async fn open_quests(
    Extension(db): Extension<DbConnection>,
) -> Result<Json<Vec<QuestSummary>>, StatusCode> {
    db.list_open_quests().await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(serde::Deserialize)]
struct JoinQuestRequest { quest_id: i32, user_id: i32 }

async fn join_quest(
    Extension(db): Extension<DbConnection>,
    Json(req): Json<JoinQuestRequest>,
) -> Result<Json<Quest>, StatusCode> {
    db.join_quest(req.quest_id, req.user_id).await.map(Json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(serde::Deserialize)]
struct CompleteQuestRequest {
    quest_id: i32,
    user_id: i32,
}

async fn get_quest(
    Extension(db): Extension<DbConnection>,
    Path(id): Path<i32>,
) -> Result<Json<Quest>, StatusCode> {
    db.get_quest_by_id(id).await.map(Json).ok_or(StatusCode::NOT_FOUND)
}

#[derive(serde::Deserialize)]
struct UpdateEncountersRequest {
    current_encounter: i32,
    current_node_id: Option<String>,
    encounters: Vec<Encounter>,
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

async fn get_dialogue(Path(id): Path<String>) -> Result<Json<Dialogue>, StatusCode> {
    let dialogues: &[(&str, &str)] = &[(
        "shady_rat",
        include_str!("../data/dialogues/shady_rat.json"),
    )];
    dialogues
        .iter()
        .find(|(name, _)| *name == id)
        .and_then(|(_, json)| serde_json::from_str(json).ok())
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

fn make_encounters() -> Vec<Encounter> {
    let encounters = {
        let mut rng = rand::thread_rng();
        let mut encounters = Vec::new();
        for i in 0..MAX_ENCOUNTER_LENGTH {
            let encounter = if i % 2 == 0 {
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
            } else {
                Encounter::NpcEncounter("shady_rat".to_string())
            };
            encounters.push(encounter);
        }
        encounters
    };

    encounters
}
