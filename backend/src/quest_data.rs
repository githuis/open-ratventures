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
    pub required_renown: u32,
}

impl Monster {
    /// Level 1 = 1–15 hp, +1 per 15 hp after that.
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
    #[serde(default)]
    pub required_renown: u32,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, sqlx::FromRow)]
pub struct Party {
    pub id: i32,
    pub leader_id: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PartySummary {
    pub id: i32,
    pub member_count: i32,
}

#[derive(Deserialize)]
pub struct JoinPartyRequest {
    pub party_id: i32,
    pub user_id: i32,
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

#[derive(Deserialize)]
pub struct JoinQuestRequest {
    pub quest_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct CompleteQuestRequest {
    pub quest_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct UpdateEncountersRequest {
    pub current_encounter: i32,
    pub current_node_id: Option<String>,
    pub encounters: Vec<Encounter>,
}
