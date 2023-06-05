#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Tiberius(#[from] tiberius::error::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}