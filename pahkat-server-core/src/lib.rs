pub mod package;
pub mod repo;

pub(crate) mod fbs {
    butte_build::include_fbs!("index");
}

pub trait Request {
    type Error;
    type Partial;

    fn new_from_user_input(partial: Self::Partial) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

pub fn make_lang_tag_map(value: String) -> pahkat_types::LangTagMap<String> {
    let mut map = pahkat_types::LangTagMap::new();
    map.insert("en".into(), value);
    map
}
