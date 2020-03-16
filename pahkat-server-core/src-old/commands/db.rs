use pahkat_common::database::{user::create_user as db_create_user, Database};

pub fn create_user(username: &str, token: &str, db_path: Option<String>) {
    let database = match Database::new(&db_path.unwrap_or_else(|| {
        pahkat_common::db_path()
            .as_path()
            .to_str()
            .unwrap()
            .to_owned()
    })) {
        Ok(database) => database,
        Err(e) => {
            panic!("Failed to create database: {}", e);
        }
    };

    let _result = db_create_user(&database, username, token).unwrap();
    println!("Added user {}", username);
}
