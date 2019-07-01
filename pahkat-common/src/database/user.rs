use uuid::Uuid;

use super::super::DatabaseError;
use super::Database;

use crate::database::models::NewUser;

pub fn create_user(
    database: &Database,
    username: &str,
    token: &str,
) -> std::result::Result<usize, DatabaseError> {
    let parsed_uuid = Uuid::parse_str(token);

    match parsed_uuid {
        Ok(uuid) => {
            let user = NewUser {
                username: username.to_owned(),
                token: uuid.as_bytes().iter().cloned().collect(),
            };

            database.create_user(user)
        }
        Err(err) => Err(DatabaseError::InputError(
            "The supplied UUID token is invalid: make sure it is in the v4 format".to_owned(),
            err,
        )),
    }
}
