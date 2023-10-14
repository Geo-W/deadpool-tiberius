use deadpool::managed::PoolError;

pub type SqlServerResult<T> = Result<T, SqlServerError>;

#[derive(Debug, thiserror::Error)]
pub enum SqlServerError {
    #[error(transparent)]
    Tiberius(#[from] tiberius::error::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Pool(#[from] PoolError<tiberius::error::Error>),
}