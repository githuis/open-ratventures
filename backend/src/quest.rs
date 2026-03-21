use axum::{
    Extension, Json, Router,
    extract::Path,
    http::StatusCode,
    routing::{get, post},
};
use rand::Rng;

use crate::data::{MAX_ENCOUNTER_LENGTH, SharedState, Unit};
use crate::db::DbConnection;
use crate::quest_data::{Combat, Dialogue, Encounter, Quest};

pub fn routes() -> Router {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/dialogue/{id}", get(get_dialogue))
}

async fn init_quest(
    Extension(db): Extension<DbConnection>,
    Json(user_id): Json<i32>,
) -> Json<Quest> {
    if let Some(existing) = db.get_quest_for_user(user_id).await {
        return Json(existing);
    }

    let quest = db.new_quest(make_encounters()).await.unwrap();
    Json(quest)
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
