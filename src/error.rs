use deadpool::managed::{BuildError, PoolError};

/// Type aliasing for Result<T,[`SqlServerError`]>
pub type SqlServerResult<T> = Result<T, SqlServerError>;

/// Error type represents error from building pool, running pool, tiberius execution, io.
#[derive(Debug, thiserror::Error)]
pub enum SqlServerError {
    /// Error caused by tiberius execution.
    #[error(transparent)]
    Tiberius(#[from] tiberius::error::Error),
    /// Error caused by io.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Error from [`PoolError`].
    #[error(transparent)]
    Pool(#[from] PoolError<tiberius::error::Error>),
    /// Error from when building pool.
    #[error(transparent)]
    PoolBuild(#[from] BuildError<tiberius::error::Error>),
}