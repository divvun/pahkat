use chrono::naive::NaiveDateTime;

use super::schema::downloads;

#[derive(Queryable)]
pub struct Download {
    pub id: Vec<u8>,

    pub package_id: String,

    pub package_version: String,

    pub timestamp: NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "downloads"]
pub struct NewDownload {
    pub id: Vec<u8>,

    pub package_id: String,

    pub package_version: String,

    pub timestamp: NaiveDateTime,
}
