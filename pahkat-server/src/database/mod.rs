use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};
use diesel::sqlite::SqliteConnection;
use failure::Error;
use log::warn;

pub mod models;
pub mod schema;

use self::models::{Download, NewDownload};
use self::schema::downloads;

#[derive(Clone)]
pub struct Database {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, PoolError> {
        let manager = ConnectionManager::<SqliteConnection>::new(path);
        let pool = Pool::builder().build(manager)?;

        Ok(Database { pool })
    }

    pub fn query_downloads(&self) -> Result<Vec<Download>, Error> {
        let connection = self.pool.get()?;

        Ok(downloads::table.load(&connection)?)
    }

    pub fn create_download(
        &self,
        download: NewDownload,
    ) -> std::result::Result<usize, Error> {
        let connection = self.pool.get()?;

        warn!("Creating Dowload");
        warn!("{:?}", &download);

        Ok(diesel::insert_into(downloads::table)
            .values(&download)
            .execute(&connection)?)
    }
}
