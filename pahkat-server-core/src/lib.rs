pub mod repo;

pub trait Request {
    type Error;
    type Partial;

    fn new_from_user_input(partial: Self::Partial) -> Result<Self, Self::Error>
    where
        Self: Sized;
}
