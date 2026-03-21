use axum::{Extension, Json, Router, extract::State, routing::post};
use rand::Rng;
use serde_json::json;

use crate::data::{Character, MAX_ENCOUNTER_LENGTH, ServerState, SharedState, Unit};
use crate::db::{self, DbConnection};
use crate::quest_data::{Combat, Encounter, EncounterReward, Quest};

pub fn routes() -> Router {
    Router::new()
        .route("/quest", post(init_quest))
        .route("/combat", post(init_combat))
}

async fn init_quest(Extension(db): Extension<DbConnection>, Json(user_id): Json<i32>) -> Json<Quest> {
    if let Some(existing) = db.get_quest_for_user(user_id).await {
        return Json(existing);
    }

    let quest = db.new_quest(make_encounters()).await.unwrap();
    Json(quest)
}

async fn init_combat(Extension(state): Extension<SharedState>) -> Json<Quest> {
    Json(Quest::default())
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
                    Encounter::NpcEncounter(EncounterReward::CoinAndExperienceReward(10, 20))
                };
                encounters.push(encounter);
            }
            encounters
        };

        encounters
}
