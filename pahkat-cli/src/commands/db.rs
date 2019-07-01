use pahkat_common::{db_path, database::{Database, user::create_user as db_create_user}};

pub fn create_user(username: &str, token: &str) {
    let database = match Database::new(db_path().as_path().to_str().unwrap()) {
        Ok(database) => database,
        Err(e) => {
            panic!("Failed to create database: {}", e);
        }
    };

    let _result = db_create_user(&database, username, token).unwrap();
    println!("Added user {}", username);
}
