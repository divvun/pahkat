table! {
    downloads (id) {
        id -> Binary,
        package_id -> Text,
        package_version -> Text,
        timestamp -> Timestamp,
    }
}

table! {
    user_access (id) {
        id -> Binary,
        user_id -> Binary,
        timestamp -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Binary,
        username -> Text,
        token -> Binary,
    }
}

joinable!(user_access -> users (user_id));

allow_tables_to_appear_in_same_query!(downloads, user_access, users,);
