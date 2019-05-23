use uuid::Uuid;

use chrono::naive::NaiveDateTime;

use crate::database::models::NewDownload;

pub struct Download {
    pub id: Uuid,

    pub package_id: String,

    pub package_version: String,

    pub timestamp: NaiveDateTime,
}

impl From<Download> for NewDownload {
    fn from(item: Download) -> Self {
        NewDownload {
            id: item.id.as_bytes().to_vec(),
            package_id: item.package_id,
            package_version: item.package_version,
            timestamp: item.timestamp,
        }
    }
}
