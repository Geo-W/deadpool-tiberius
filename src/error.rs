use deadpool::managed::PoolError;

pub type SqlServerResult<T> = Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Tiberius(#[from] tiberius::error::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<PoolError<Error>> for Error {
    fn from(err: PoolError<Error>) -> Self {
        err.into()
    }
}
