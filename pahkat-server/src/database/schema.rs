table! {
    downloads (id) {
        id -> Binary,
        package_id -> Text,
        package_version -> Text,
        timestamp -> Timestamp,
    }
}
