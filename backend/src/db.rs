use color_eyre::Result;
use std::str::FromStr;
use std::{env, error::Error, fmt::Debug};

use sqlx::Connection;
use sqlx::SqliteConnection;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::data::{Character, CharacterWrapper, Unit, User};

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
}
