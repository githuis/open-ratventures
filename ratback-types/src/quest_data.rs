use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::data::{Item, Unit};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Monster {
    pub unit: Unit,
    pub name: String,
    pub attack: i32,
    pub items: Vec<Item>,
    #[serde(default)]
    pub areas: Vec<String>,
}

impl Monster {
    pub fn level(&self) -> u32 {
        (self.unit.max_health as u32 + 14) / 15
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Combat {
    pub monsters: Vec<Monster>,
    pub turn: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum Encounter {
    #[default]
    EmptyEncounter,
    CombatEncounter(Combat),
    NpcEncounter(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum EncounterReward {
    #[default]
    NoReward,
    CoinReward(u32),
    ExperienceReward(u32),
    CoinAndExperienceReward(u32, u32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dialogue {
    pub id: String,
    pub start: String,
    pub nodes: HashMap<String, DialogueNode>,
    #[serde(default)]
    pub areas: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueNode {
    pub text: String,
    pub choices: Vec<DialogueChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueChoice {
    pub text: String,
    pub next: Option<String>,
    pub outcome: Option<DialogueOutcome>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DialogueOutcome {
    Reward { coins: i32, renown: i32, #[serde(default)] heal: i32 },
    Damage { amount: i32 },
    Combat(Combat),
    GiveItem { item_name: String, cost: i32 },
    Escape,
    NextEncounter,
    GiveClue { clue_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissionDef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub clue_id: String,
    pub encounters: Vec<Encounter>,
    pub completion_reward: MissionReward,
    pub is_final: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MissionReward {
    pub coins: i32,
    pub renown: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissionStatus {
    pub mission_id: String,
    pub title: String,
    pub description: String,
    pub state: MissionState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MissionState {
    Locked,
    Ready,
    InProgress,
    Complete,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow, sqlx::Type))]
pub struct Quest {
    pub id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(skip))]
    pub encounters: Vec<Encounter>,
    pub current_encounter: i32,
    #[serde(default)]
    pub current_node_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Party {
    pub id: i32,
    pub leader_id: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PartySummary {
    pub id: i32,
    pub member_count: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestSummary {
    pub id: i32,
    pub member_count: i32,
}
