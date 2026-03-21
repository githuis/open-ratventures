use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::data::{Character, Item, MAX_COMBAT_ENEMIES, MAX_ENCOUNTER_LENGTH, MAX_PARTY_SIZE, Unit};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Combat {
    pub monsters: Vec<Unit>,
    pub turn: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum Encounter {
    #[default]
    EmptyEncounter,
    CombatEncounter(Combat),
    NpcEncounter(String), // dialogue_id
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum EncounterReward {
    #[default]
    NoReward,
    CoinReward(u32),
    ExperienceReward(u32),
    CoinAndExperienceReward(u32, u32),
}

// --- Dialogue types ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dialogue {
    pub id: String,
    pub start: String,
    pub nodes: HashMap<String, DialogueNode>,
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
    Reward { coins: u32, experience: u32 },
    Combat(Combat),
    Escape,
    NextEncounter,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, sqlx::FromRow, sqlx::Type)]
pub struct Quest {
    pub id: i32,
    #[sqlx(skip)]
    pub encounters: Vec<Encounter>,
    pub current_encounter: i32,
    #[serde(default)]
    pub current_node_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, sqlx::FromRow, sqlx::Type)]
pub struct Party {
    pub members: Vec<i32>, //Character id
    pub quest_id: i32,
}

pub enum CombatAction {
    WeaponAttack,
    UseItem(Item),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestSummary {
    pub id: i32,
    pub member_count: i32,
}
