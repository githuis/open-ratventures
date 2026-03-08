use std::{error::Error, fmt::Debug};


use crate::data::User;

/*
pub struct DbConnection {
    conn: SqliteConnection
}

impl DbConnection {

    pub fn new(&mut self, db_name: &str) -> DbConnection{
        DbConnection {
            conn: SqliteConnection::establish(db_name).unwrap_or_else(|_| panic!("error connecting to db")),
        }
    }

    pub fn persist_user(&self, user: &User) -> Result<User, Box<dyn Error>> {
        let sql = match user.id {
            0  => format!(  "INSERT INTO user (username) values ({});", user.username).to_string(),
            _ => "update user (id, username, characters) where ".to_string(),
            
        };



    }

}
 */