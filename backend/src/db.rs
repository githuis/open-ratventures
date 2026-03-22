use color_eyre::Result;
use rand::Rng;
use std::str::FromStr;
use std::{env, error::Error, fmt::Debug};

use sqlx::Connection;
use sqlx::SqliteConnection;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::data::{
    Character, CharacterWrapper, InventoryItem, Item, ItemEffect, MAX_ENCOUNTER_LENGTH, Unit, User,
    random_fantasy_name,
};
use crate::quest_data::QuestSummary;
use crate::quest_data::{Combat, Encounter, Quest};

#[derive(Clone)]
pub struct DbConnection {
    pub pool: SqlitePool,
}

impl DbConnection {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let db_options = SqliteConnectOptions::from_str("sqlite://data.db")?.to_owned();

        let pool = SqlitePoolOptions::new().connect_with(db_options).await?;

        println!("Checking for migrations..");
        sqlx::migrate!("./migrations").run(&pool).await?;
        println!("Finished running migrating db");

        // On startup, wipe in-progress quests so lobbies start clean each session
        sqlx::query("DELETE FROM quests WHERE status = 'active'")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    //pub fn persist_user(&self, user: &User) -> Result<User, Box<dyn Error>> {
    //let sql = match user.id {
    //0  => format!(  "INSERT INTO user (username) values ({});", user.username).to_string(),
    //_ => "update user (id, username, characters) where ".to_string(),

    //};

    //}

    pub async fn register_user(&self, username: String) -> Result<User> {
        if let Ok(existing) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
            .bind(&username)
            .fetch_one(&self.pool)
            .await
        {
            println!("Fetched existing user: {}", existing.username);
            return Ok(existing);
        }

        let id = sqlx::query("INSERT INTO users (username) VALUES ($1)")
            .bind(&username)
            .execute(&self.pool)
            .await?
            .last_insert_rowid();

        println!("Registered new user: {}", username);
        Ok(User {
            id: id as i32,
            username,
        })
    }

    pub async fn get_character(&self, user_id: String) -> Result<CharacterWrapper> {
        let user_id: i32 = user_id.parse()?;

        let existing =
            sqlx::query_as::<_, Character>("SELECT * FROM characters WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;

        let character = match existing {
            Some(c) => c,
            None => self.create_character(user_id).await?,
        };

        let unit = sqlx::query_as::<_, Unit>("SELECT * FROM units WHERE ref_id = $1")
            .bind(character.id)
            .fetch_one(&self.pool)
            .await?;

        Ok(CharacterWrapper { character, unit })
    }

    async fn create_character(&self, user_id: i32) -> Result<Character> {
        let name = random_fantasy_name();
        let char_id = sqlx::query(
            "INSERT INTO characters (user_id, name, experience, coins) VALUES ($1, $2, 0, 0)",
        )
        .bind(user_id)
        .bind(name)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        sqlx::query(
            "INSERT INTO units (ref_id, health, energy, max_health, max_energy) VALUES ($1, 15, 10, 15, 10)",
        )
        .bind(char_id)
        .execute(&self.pool)
        .await?;

        Ok(
            sqlx::query_as::<_, Character>("SELECT * FROM characters WHERE id = $1")
                .bind(char_id as i32)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn get_character_items(&self, user_id: i32) -> Result<Vec<InventoryItem>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i32,
            name: String,
            description: String,
            effect_type: String,
            effect_value: i32,
            charges: i32,
            charges_remaining: i32,
        }

        let rows = sqlx::query_as::<_, Row>(
            "SELECT i.id, i.name, i.description, i.effect_type, i.effect_value, i.charges, ci.charges_remaining
             FROM character_items ci
             JOIN items i ON ci.item_id = i.id
             JOIN characters c ON c.id = ci.character_id
             WHERE c.user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let effect = match r.effect_type.as_str() {
                    "heal" => ItemEffect::Heal(r.effect_value),
                    "full_heal" => ItemEffect::FullHeal,
                    _ => ItemEffect::Damage(r.effect_value),
                };
                InventoryItem {
                    item: Item {
                        id: r.id,
                        name: r.name,
                        description: r.description,
                        effect,
                        charges: r.charges,
                    },
                    charges_remaining: r.charges_remaining,
                }
            })
            .collect())
    }

    pub async fn give_item_to_character(&self, user_id: i32, item_name: &str) -> Result<()> {
        let char_id: i32 = sqlx::query_scalar("SELECT id FROM characters WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        #[derive(sqlx::FromRow)]
        struct ItemRow { id: i32, charges: i32 }
        let item = sqlx::query_as::<_, ItemRow>("SELECT id, charges FROM items WHERE name = $1 LIMIT 1")
            .bind(item_name)
            .fetch_one(&self.pool)
            .await?;

        // On conflict: add charges (or keep -1 for infinite)
        sqlx::query(
            "INSERT INTO character_items (character_id, item_id, charges_remaining) VALUES ($1, $2, $3)
             ON CONFLICT(character_id, item_id) DO UPDATE SET
               charges_remaining = CASE WHEN $3 = -1 THEN -1 ELSE charges_remaining + $3 END",
        )
        .bind(char_id)
        .bind(item.id)
        .bind(item.charges)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn consume_item(&self, user_id: i32, item_id: i32) -> Result<()> {
        let char_id: i32 = sqlx::query_scalar("SELECT id FROM characters WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        // Decrement charges unless infinite (-1)
        sqlx::query(
            "UPDATE character_items
             SET charges_remaining = CASE WHEN charges_remaining = -1 THEN -1 ELSE charges_remaining - 1 END
             WHERE character_id = $1 AND item_id = $2",
        )
        .bind(char_id)
        .bind(item_id)
        .execute(&self.pool)
        .await?;

        // Remove when all charges spent
        sqlx::query("DELETE FROM character_items WHERE character_id = $1 AND item_id = $2 AND charges_remaining = 0")
            .bind(char_id)
            .bind(item_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_quest_by_id(&self, quest_id: i32) -> Option<Quest> {
        #[derive(sqlx::FromRow)]
        struct QuestRow {
            id: i32,
            current_encounter: i32,
            encounters_json: String,
            current_node: Option<String>,
        }

        let row = sqlx::query_as::<_, QuestRow>(
            "SELECT id, current_encounter, encounters_json, current_node FROM quests WHERE id = $1 AND status = 'active'",
        )
        .bind(quest_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()?;

        let encounters: Vec<Encounter> =
            serde_json::from_str(&row.encounters_json).unwrap_or_default();
        Some(Quest {
            id: row.id,
            current_encounter: row.current_encounter,
            encounters,
            current_node_id: row.current_node,
        })
    }

    pub async fn get_quest_for_user(&self, user_id: i32) -> Option<Quest> {
        #[derive(sqlx::FromRow)]
        struct QuestRow {
            id: i32,
            current_encounter: i32,
            encounters_json: String,
            current_node: Option<String>,
        }

        let row = sqlx::query_as::<_, QuestRow>(
            "SELECT q.id, q.current_encounter, q.encounters_json, q.current_node
             FROM quests q
             JOIN quest_members qm ON q.id = qm.quest_id
             JOIN characters c ON c.id = qm.character_id
             WHERE c.user_id = $1 AND q.status = 'active'
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()?;

        let encounters: Vec<Encounter> =
            serde_json::from_str(&row.encounters_json).unwrap_or_default();
        Some(Quest {
            id: row.id,
            current_encounter: row.current_encounter,
            encounters,
            current_node_id: row.current_node,
        })
    }

    pub async fn list_open_quests(&self) -> Result<Vec<QuestSummary>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i32,
            member_count: i64,
        }

        let rows = sqlx::query_as::<_, Row>(
            "SELECT q.id, COUNT(qm.character_id) as member_count
             FROM quests q
             LEFT JOIN quest_members qm ON q.id = qm.quest_id
             WHERE q.status = 'active'
             GROUP BY q.id
             HAVING COUNT(qm.character_id) BETWEEN 1 AND $1 - 1",
        )
        .bind(crate::data::MAX_PARTY_SIZE as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| QuestSummary {
                id: r.id,
                member_count: r.member_count as i32,
            })
            .collect())
    }

    pub async fn join_quest(&self, quest_id: i32, user_id: i32) -> Result<Quest> {
        let character_id: i32 = sqlx::query_scalar("SELECT id FROM characters WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        let slot: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM quest_members WHERE quest_id = $1")
                .bind(quest_id)
                .fetch_one(&self.pool)
                .await?;

        sqlx::query("INSERT OR IGNORE INTO quest_members (quest_id, character_id, slot_index) VALUES ($1, $2, $3)")
            .bind(quest_id)
            .bind(character_id)
            .bind(slot as i32)
            .execute(&self.pool)
            .await?;

        self.get_quest_for_user(user_id)
            .await
            .ok_or_else(|| color_eyre::eyre::eyre!("quest not found after join"))
    }

    pub async fn new_quest(&self, encounters: Vec<Encounter>, user_id: i32) -> Result<Quest> {
        let encounters_json = serde_json::to_string(&encounters)?;
        let id =
            sqlx::query("INSERT INTO quests (current_encounter, encounters_json) VALUES (0, $1)")
                .bind(&encounters_json)
                .execute(&self.pool)
                .await?
                .last_insert_rowid();

        self.join_quest(id as i32, user_id).await?;

        Ok(Quest {
            id: id as i32,
            encounters,
            current_encounter: 0,
            current_node_id: None,
        })
    }

    pub async fn get_quest_members(&self, quest_id: i32) -> Result<Vec<CharacterWrapper>> {
        let characters = sqlx::query_as::<_, Character>(
            "SELECT c.* FROM characters c
             JOIN quest_members qm ON c.id = qm.character_id
             WHERE qm.quest_id = $1",
        )
        .bind(quest_id)
        .fetch_all(&self.pool)
        .await?;

        let mut members = Vec::new();
        for character in characters {
            let unit = sqlx::query_as::<_, Unit>("SELECT * FROM units WHERE ref_id = $1")
                .bind(character.id)
                .fetch_one(&self.pool)
                .await?;
            members.push(CharacterWrapper { character, unit });
        }
        Ok(members)
    }

    pub async fn update_quest_encounters(
        &self,
        quest_id: i32,
        current_encounter: i32,
        current_node: Option<String>,
        encounters: Vec<Encounter>,
    ) -> Result<()> {
        let json = serde_json::to_string(&encounters)?;
        sqlx::query("UPDATE quests SET encounters_json = $1, current_encounter = $2, current_node = $3 WHERE id = $4")
            .bind(json)
            .bind(current_encounter)
            .bind(current_node)
            .bind(quest_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn complete_quest(&self, quest_id: i32, user_id: i32) -> Result<CharacterWrapper> {
        sqlx::query("UPDATE quests SET status = 'completed' WHERE id = $1")
            .bind(quest_id)
            .execute(&self.pool)
            .await?;

        // Reward all members of the quest
        sqlx::query(
            "UPDATE characters SET experience = experience + 15, coins = coins + 5
             WHERE id IN (SELECT character_id FROM quest_members WHERE quest_id = $1)",
        )
        .bind(quest_id)
        .execute(&self.pool)
        .await?;

        let character =
            sqlx::query_as::<_, Character>("SELECT * FROM characters WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;

        let unit = sqlx::query_as::<_, Unit>("SELECT * FROM units WHERE ref_id = $1")
            .bind(character.id)
            .fetch_one(&self.pool)
            .await?;

        Ok(CharacterWrapper { character, unit })
    }

    pub async fn update_unit_for_user(&self, user_id: i32, unit: &Unit) -> Result<()> {
        let char_id: i32 = sqlx::query_scalar("SELECT id FROM characters WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
        sqlx::query(
            "UPDATE units SET health = $1, energy = $2, max_health = $3, max_energy = $4 WHERE ref_id = $5",
        )
        .bind(unit.health)
        .bind(unit.energy)
        .bind(unit.max_health)
        .bind(unit.max_energy)
        .bind(char_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_character_by_user_id(&self, user_id: i32) -> Result<CharacterWrapper> {
        let character =
            sqlx::query_as::<_, Character>("SELECT * FROM characters WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;

        let unit = sqlx::query_as::<_, Unit>("SELECT * FROM units WHERE ref_id = $1")
            .bind(character.id)
            .fetch_one(&self.pool)
            .await?;

        Ok(CharacterWrapper { character, unit })
    }
}
