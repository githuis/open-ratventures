use color_eyre::Result;
use rand::Rng;
use std::str::FromStr;
use std::{env, error::Error, fmt::Debug};

use sqlx::Connection;
use sqlx::SqliteConnection;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::data::{Character, CharacterWrapper, MAX_ENCOUNTER_LENGTH, Unit, User};
use crate::quest_data::{Combat, Encounter, EncounterReward, Quest};

#[derive(Clone)]
pub struct DbConnection {
    pub pool: SqlitePool,
}

impl DbConnection {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let db_options = SqliteConnectOptions::from_str("sqlite://data.db")?.to_owned();

        let pool = SqlitePoolOptions::new().connect_with(db_options).await?;

        Ok(Self { pool })
    }

    //pub fn persist_user(&self, user: &User) -> Result<User, Box<dyn Error>> {
    //let sql = match user.id {
    //0  => format!(  "INSERT INTO user (username) values ({});", user.username).to_string(),
    //_ => "update user (id, username, characters) where ".to_string(),

    //};

    //}

    pub async fn register_user(&self, username: String) -> Result<User> {
        let mut stream = sqlx::query_as::<_, User>("SELECT * FROM users where username = $1")
            .bind(username.clone())
            .fetch_one(&self.pool)
            .await;

        let new_user = match stream {
            Ok(u) => {
                println!("Fetched user from database {}", u.username);
                u
            }
            Err(e) => {
                println!(
                    "Couldn't find user with name {}, registered new user. Msg: {}",
                    username.clone(),
                    e.to_string()
                );

                User {
                    username: username,
                    ..Default::default()
                }
            }
        };

        println!("Registered user: {}", new_user.clone().username);

        Ok(new_user)
    }

    pub async fn get_character(&self, user_id: String) -> Result<CharacterWrapper> {
        let stream = sqlx::query_as::<_, Character>("Select * from characters where user_id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        let stats = sqlx::query_as::<_, Unit>("select * from units where ref_id = $1")
            .bind(stream.id)
            .fetch_one(&self.pool)
            .await?;

        println!("Returning character for user");
        Ok(CharacterWrapper {
            unit: stats,
            character: stream,
        })
    }

    pub async fn get_quest_for_user(&self, user_id: i32) -> Option<Quest> {
        sqlx::query_as::<_, Quest>(
            "SELECT q.id, q.current_encounter
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
        .flatten()
    }

    pub async fn new_quest(&self) -> Result<Quest> {
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

        let id = sqlx::query("INSERT INTO quests (current_encounter) VALUES (0)")
            .execute(&self.pool)
            .await?
            .last_insert_rowid();

        Ok(Quest {
            id: id as i32,
            encounters,
            current_encounter: 0,
        })
    }
}
