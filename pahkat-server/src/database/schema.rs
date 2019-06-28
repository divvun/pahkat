table! {
    downloads (id) {
        id -> Binary,
        package_id -> Text,
        package_version -> Text,
        timestamp -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Binary,
        name -> Text,
        token -> Binary,
    }
}

allow_tables_to_appear_in_same_query!(
    downloads,
    users,
);
