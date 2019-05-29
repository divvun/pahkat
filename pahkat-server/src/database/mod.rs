use chrono::offset::Utc;
use chrono::{Duration, NaiveDateTime};
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};
use diesel::sqlite::SqliteConnection;
use failure::Error;

use pahkat_types::Package;

pub mod models;
pub mod schema;

use self::models::{NewDownload, PackageCount};
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

    pub fn query_package_download_count(&self, package: &Package) -> Result<i64, Error> {
        use self::schema::downloads::dsl::*;
        use diesel::dsl::count;

        let connection = self.pool.get()?;

        Ok(downloads
            .filter(package_id.eq(&package.id))
            .select(count(package_id))
            .first(&connection)?)
    }

    pub fn query_package_version_download_count(&self, package: &Package) -> Result<i64, Error> {
        use self::schema::downloads::dsl::*;
        use diesel::dsl::count;

        let connection = self.pool.get()?;

        Ok(downloads
            .filter(
                package_id
                    .eq(&package.id)
                    .and(package_version.eq(&package.version)),
            )
            .select(count(package_id))
            .first(&connection)?)
    }

    pub fn query_package_download_count_since(
        &self,
        package: &Package,
        duration: Duration,
    ) -> Result<i64, Error> {
        use self::schema::downloads::dsl::*;
        use diesel::dsl::count;

        let connection = self.pool.get()?;

        Ok(downloads
            .filter(package_id.eq(&package.id))
            .filter(timestamp.ge(Database::get_bound(duration)))
            .select(count(package_id))
            .first(&connection)?)
    }

    pub fn query_top_downloads(&self, limit: u32) -> std::result::Result<Vec<PackageCount>, Error> {
        use diesel::sql_query;
        use diesel::sql_types::Integer;

        let connection = self.pool.get()?;

        // `?` is SQLite notation for a query parameter. Would need to be `$1` and so on in Postgres
        Ok(sql_query(
            r#"
SELECT package_id, COUNT(package_id) as count
FROM downloads
GROUP BY package_id
ORDER BY count DESC
LIMIT ?
"#,
        )
        .bind::<Integer, _>(limit as i32)
        .load(&connection)?)
    }

    pub fn query_top_downloads_since(
        &self,
        limit: u32,
        duration: Duration,
    ) -> std::result::Result<Vec<PackageCount>, Error> {
        use diesel::sql_query;
        use diesel::sql_types::{Integer, Timestamp};

        let connection = self.pool.get()?;

        // `?` is SQLite notation for a query parameter. Would need to be `$1` and so on in Postgres
        Ok(sql_query(
            r#"
SELECT package_id, COUNT(package_id) as count
FROM downloads
WHERE timestamp > ?
GROUP BY package_id
ORDER BY count DESC
LIMIT ?
"#,
        )
        .bind::<Timestamp, _>(Database::get_bound(duration))
        .bind::<Integer, _>(limit as i32)
        .load(&connection)?)
    }

    pub fn query_distinct_downloads_since(
        &self,
        duration: Duration,
    ) -> std::result::Result<Vec<String>, Error> {
        use self::schema::downloads::dsl::*;

        let connection = self.pool.get()?;

        Ok(downloads
            .select(package_id)
            .filter(timestamp.ge(Database::get_bound(duration)))
            .distinct()
            .load(&connection)?)
    }

    pub fn create_download(&self, download: NewDownload) -> std::result::Result<usize, Error> {
        let connection = self.pool.get()?;

        Ok(diesel::insert_into(downloads::table)
            .values(&download)
            .execute(&connection)?)
    }

    fn get_bound(duration: Duration) -> NaiveDateTime {
        let now = Utc::now().naive_utc();
        now.checked_sub_signed(duration)
            .expect("Date subtraction overflowed when retrieving download count")
    }
}
