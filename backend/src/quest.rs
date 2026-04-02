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
use crate::quest_data::{Combat, CompleteQuestRequest, Dialogue, Encounter, GiveClueRequest, JoinQuestRequest, MissionDef, MissionState, MissionStatus, Monster, Quest, QuestSummary, UpdateEncountersRequest};

#[derive(serde::Deserialize)]
struct NewQuestRequest {
    user_id: i32,
    area: String,
    mission_id: Option<String>,
}

type DialogueMap = Arc<HashMap<String, Dialogue>>;
type EnemyList = Arc<Vec<Monster>>;
type MissionList = Arc<Vec<MissionDef>>;

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
        .route("/missions/{character_id}", get(get_missions))
        .route("/clue", post(give_clue))
}

async fn init_quest(
    Extension(db): Extension<DbConnection>,
    Extension(dialogues): Extension<DialogueMap>,
    Extension(enemies): Extension<EnemyList>,
    Extension(missions): Extension<MissionList>,
    Json(req): Json<NewQuestRequest>,
) -> Json<Quest> {
    if let Some(existing) = db.get_quest_for_user(req.user_id).await {
        return Json(existing);
    }
    let quest = if let Some(ref mid) = req.mission_id {
        if let Some(mission) = missions.iter().find(|m| &m.id == mid) {
            db.new_mission_quest(mission.encounters.clone(), req.user_id, mid).await.unwrap()
        } else {
            db.new_quest(make_encounters(&dialogues, &enemies, &req.area), req.user_id).await.unwrap()
        }
    } else {
        db.new_quest(make_encounters(&dialogues, &enemies, &req.area), req.user_id).await.unwrap()
    };
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
    Extension(missions): Extension<MissionList>,
    Json(req): Json<CompleteQuestRequest>,
) -> Result<Json<CharacterWrapper>, StatusCode> {
    // Check if this is a mission quest
    let mission_id: Option<String> = sqlx::query_scalar("SELECT mission_id FROM quests WHERE id = $1")
        .bind(req.quest_id)
        .fetch_optional(&db.pool)
        .await
        .ok()
        .flatten()
        .flatten();

    let (reward_coins, reward_renown) = if let Some(ref mid) = mission_id {
        missions.iter().find(|m| &m.id == mid)
            .map(|m| (m.completion_reward.coins as u32, m.completion_reward.renown as u32))
            .unwrap_or((5, 1))
    } else {
        (5, 1)
    };

    let result = db.complete_quest(req.quest_id, req.user_id, reward_coins, reward_renown)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(ref mid) = mission_id {
        let char_id = result.character.id;
        db.complete_mission(char_id, mid).await.ok();
    }

    Ok(Json(result))
}

async fn get_missions(
    Extension(db): Extension<DbConnection>,
    Extension(missions): Extension<MissionList>,
    Path(character_id): Path<i32>,
) -> Json<Vec<MissionStatus>> {
    let clues = db.get_clues(character_id).await.unwrap_or_default();
    let states = db.get_mission_states(character_id).await.unwrap_or_default();

    let statuses = missions.iter().map(|m| {
        let row = states.iter().find(|(mid, ..)| mid == &m.id);
        let state = match row {
            Some((_, true, _, _)) => MissionState::Complete,
            Some((_, false, true, _)) => MissionState::InProgress,
            _ => {
                if clues.contains(&m.clue_id) {
                    MissionState::Ready
                } else {
                    MissionState::Locked
                }
            }
        };
        MissionStatus {
            mission_id: m.id.clone(),
            title: m.title.clone(),
            description: m.description.clone(),
            state,
        }
    }).collect();

    Json(statuses)
}

async fn give_clue(
    Extension(db): Extension<DbConnection>,
    Extension(missions): Extension<MissionList>,
    Json(req): Json<GiveClueRequest>,
) -> Result<Json<Option<MissionStatus>>, StatusCode> {
    let newly_granted = db.give_clue(req.character_id, &req.clue_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !newly_granted {
        return Ok(Json(None));
    }

    // Find mission unlocked by this clue
    let mission = missions.iter().find(|m| m.clue_id == req.clue_id);
    let status = if let Some(m) = mission {
        Some(MissionStatus {
            mission_id: m.id.clone(),
            title: m.title.clone(),
            description: m.description.clone(),
            state: MissionState::Ready,
        })
    } else {
        None
    };

    Ok(Json(status))
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

fn make_encounters(dialogues: &HashMap<String, Dialogue>, enemies: &[Monster], area: &str) -> Vec<Encounter> {
    let eligible_enemies: Vec<&Monster> = enemies.iter()
        .filter(|e| e.areas.is_empty() || e.areas.iter().any(|a| a == area))
        .collect();
    let eligible_dialogues: Vec<&str> = dialogues.values()
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
