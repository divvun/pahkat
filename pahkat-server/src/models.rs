use std::fmt;

use uuid::Uuid;
use chrono::naive::NaiveDateTime;

pub struct Download {
    pub id: Uuid,

    pub package_id: String,

    pub package_version: String,

    pub timestamp: NaiveDateTime,
}

impl fmt::Display for Download {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Download {{ package_id: \"{}\", package_version: \"{}\", timestamp: \"{}\" }}",
               self.package_id, self.package_version, self.timestamp)
    }
}

impl From<crate::database::models::Download> for Download {
    fn from(item: crate::database::models::Download) -> Self {
        Download {
            id: Uuid::from_slice(&item.id).expect("Failed to convert database id value to UUID"),
            package_id: item.package_id,
            package_version: item.package_version,
            timestamp: item.timestamp,
        }
    }
}
