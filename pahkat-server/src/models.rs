use uuid::Uuid;

use chrono::naive::NaiveDateTime;

pub struct Download {
    pub id: Uuid,

    pub package_id: String,

    pub package_version: String,

    pub timestamp: NaiveDateTime,
}
