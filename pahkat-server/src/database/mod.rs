use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};
use diesel::result::Error;
use diesel::sqlite::SqliteConnection;

pub mod models;
pub mod schema;

use self::models::{Download, NewDownload};
use self::schema::downloads;

pub struct Database {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, PoolError> {
        let manager = ConnectionManager::<SqliteConnection>::new(path);
        let pool = Pool::builder().build(manager)?;

        Ok(Database { pool })
    }

    pub fn query_downloads(connection: &SqliteConnection) -> Result<Vec<Download>, Error> {
        downloads::table.load(connection)
    }

    pub fn create_download<T: Into<NewDownload>>(
        connection: &SqliteConnection,
        download: T,
    ) -> std::result::Result<usize, diesel::result::Error> {
        diesel::insert_into(downloads::table)
            .values(&download.into())
            .execute(connection)
    }
}
