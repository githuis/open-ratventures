use serde::{Serialize, Deserialize};

use crate::data::{Character, Item, MAX_COMBAT_ENEMIES, MAX_ENCOUNTER_LENGTH, MAX_PARTY_SIZE};


#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Combat {
    pub monsters: [Character; MAX_COMBAT_ENEMIES],
    pub turn: u16,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub enum Encounter {
    #[default]
    EmptyEncounter,
    CombatEncounter(Combat),
    NpcEncounter(EncounterReward),
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub enum EncounterReward{
    #[default]
    NoReward,
    CoinReward(u32),
    ExperienceReward(u32),
    CoinAndExperienceReward(u32, u32)
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Quest {
    pub members: [Character; MAX_PARTY_SIZE],
    pub encounters: [Encounter; MAX_ENCOUNTER_LENGTH],
    pub open_encounter: Option<Encounter>,
}

pub enum CombatAction {
    WeaponAttack,
    UseItem(Item)
}