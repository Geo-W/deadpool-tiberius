//! This crate chains config from [`tiberius`] and [`deadpool`] to make it easier for creating tiberius connection pool.
//! # Example
//! ```no_run
//! let pool = deadpool_tiberius::Manager::new()
//!     .host("host")
//!     .port(1433)
//!     .basic_authentication("username", "password")
//!     .database("database1")
//!     .max_size(20)
//!     .wait_timeout(1.52)
//!     .pre_recycle_sync(|_client, _metrics| {
//!         // do sth with connection object and pool metrics.
//!         Ok(())
//!     })
//!     .create_pool()?;
//! ```
//!
//! [`Manager::from_ado_string`] and [`Manager::from_jdbc_string`] also served as another entry for constructing Manager.
//! ```no_run
//! const CONN_STR: &str = "Driver={SQL Server};Integrated Security=True;\
//!                         Server=DESKTOP-TTTTTTT;Database=master;\
//!                         Trusted_Connection=yes;encrypt=DANGER_PLAINTEXT;";
//! let pool = deadpool_tiberius::Manager::from_ado_string(CONN_STR)?
//!                 .max_size(20)
//!                 .wait_timeout(1.52)
//!                 .create_pool()?;
//! ```
//! For all configurable pls visit [`Manager`].
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
use std::mem::replace;
use std::time::Duration;

pub use deadpool;
use deadpool::{
    async_trait, managed,
    managed::{Hook, HookFuture, HookResult, Metrics, PoolConfig, RecycleError, RecycleResult},
    Runtime,
};
pub use tiberius;
use tiberius::error::Error;
use tiberius::{AuthMethod, EncryptionLevel};
use tokio_util::compat::TokioAsyncWriteCompatExt;

pub use crate::error::SqlServerError;
pub use crate::error::SqlServerResult;

mod error;

/// Type aliasing for tiberius client with [`tokio`] as runtime.
pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;
/// Type aliasing for Pool.
pub type Pool = managed::Pool<Manager>;

/// Connection pool Manager served as Builder. Call [`create_pool`] after filling out your configs.
///
/// [`create_pool`]: struct.Manager.html#method.create_pool
pub struct Manager {
    config: tiberius::Config,
    pool_config: PoolConfig,
    runtime: Option<Runtime>,
    hooks: Hooks,
    modify_tcp_stream:
        Box<dyn Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static>,
    #[cfg(feature = "sql-browser")]
    enable_sql_browser: bool,
}

#[async_trait]
impl managed::Manager for Manager {
    type Type = Client;
    type Error = tiberius::error::Error;

    #[cfg(feature = "sql-browser")]
    async fn create(&self) -> Result<Client, Self::Error> {
        use tiberius::SqlBrowser;
        let tcp = if !self.enable_sql_browser {
            tokio::net::TcpStream::connect(self.config.get_addr()).await?
        } else {
            tokio::net::TcpStream::connect_named(&self.config).await?
        };
        (self.modify_tcp_stream)(&tcp)?;
        let client = Client::connect(self.config.clone(), tcp.compat_write()).await;
        match client {
            Ok(client) => Ok(client),
            Err(Error::Routing { host, port }) => {
                let mut config = self.config.clone();
                config.host(host);
                config.port(port);

                let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
                tcp.set_nodelay(true)?;

                Client::connect(config, tcp.compat_write()).await
            },
            // Propagate errors
            Err(err) => Err(err)?,
        }
    }

    #[cfg(not(feature = "sql-browser"))]
    async fn create(&self) -> Result<Client, Self::Error> {
        let tcp = tokio::net::TcpStream::connect(self.config.get_addr()).await?;
        (self.modify_tcp_stream)(&tcp)?;
        let client = Client::connect(self.config.clone(), tcp.compat_write()).await;

        match client {
            Ok(client) => Ok(client),
            Err(Error::Routing { host, port }) => {
                let mut config = self.config.clone();
                config.host(host);
                config.port(port);

                let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
                tcp.set_nodelay(true)?;

                Client::connect(config, tcp.compat_write()).await
            },
            // Propagate errors
            Err(err) => Err(err)?,
        }
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        match obj.simple_query("").await {
            Ok(_) => Ok(()),
            Err(e) => Err(RecycleError::Message(e.to_string())),
        }
    }
}

impl Manager {
    /// Create new ConnectionPool Manager
    pub fn new() -> Self {
        Self::new_with_tiberius_config(tiberius::Config::new())
    }

    /// Create a new ConnectionPool Manager and fills connection configs from ado string.
    /// For more details about ADO_String pleas refer to [`tiberius::Config::from_ado_string`] and [`Connection Strings in ADO.NET`].
    ///
    /// [`Connection Strings in ADO.NET`]: https://docs.microsoft.com/en-us/dotnet/framework/data/adonet/connection-strings
    pub fn from_ado_string(conn_str: &str) -> SqlServerResult<Self> {
        Ok(Self::new_with_tiberius_config(
            tiberius::Config::from_ado_string(conn_str)?,
        ))
    }

    /// Create new ConnectionPool Manager and fills connection config from jdbc string.
    /// For more details about jdbc_string pls refer to [`Building JDBC connection URL`].
    ///
    /// [`Building JDBC connection URL`]: https://docs.microsoft.com/en-us/sql/connect/jdbc/building-the-connection-url?view=sql-server-ver15
    pub fn from_jdbc_string(conn_str: &str) -> SqlServerResult<Self> {
        Ok(Self::new_with_tiberius_config(
            tiberius::Config::from_jdbc_string(conn_str)?,
        ))
    }

    fn new_with_tiberius_config(config: tiberius::Config) -> Self {
        Self {
            config,
            pool_config: Default::default(),
            runtime: None,
            hooks: Default::default(),
            modify_tcp_stream: Box::new(|tcp_stream| tcp_stream.set_nodelay(true)),
            #[cfg(feature = "sql-browser")]
            enable_sql_browser: false,
        }
    }

    /// Consume self, builds a pool.
    pub fn create_pool(mut self) -> Result<Pool, error::SqlServerError> {
        let config = self.pool_config;
        let runtime = self.runtime;
        let hooks = replace(&mut self.hooks, Hooks::default());
        let mut pool = Pool::builder(self).config(config);
        if let Some(v) = runtime {
            pool = pool.runtime(v);
        }

        for hook in hooks.post_create {
            pool = pool.post_create(hook);
        }
        for hook in hooks.pre_recycle {
            pool = pool.pre_recycle(hook);
        }
        for hook in hooks.post_recycle {
            pool = pool.post_recycle(hook);
        }

        Ok(pool.build()?)
    }

    /// Whether connected via sql-browser feature, default to `false`.
    #[cfg(feature = "sql-browser")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sql-browser")))]
    pub fn enable_sql_browser(mut self) -> Self {
        self.enable_sql_browser = true;
        self
    }

    /// Server host, defaults to `localhost`.
    pub fn host(mut self, host: impl ToString) -> Self {
        self.config.host(host);
        self
    }

    /// Server port, defaults to 1433.
    pub fn port(mut self, port: u16) -> Self {
        self.config.port(port);
        self
    }

    /// Database, defaults to `master`.
    pub fn database(mut self, database: impl ToString) -> Self {
        self.config.database(database);
        self
    }

    /// Simplified authentication for those using `username` and `password` as login method.
    pub fn basic_authentication(
        mut self,
        username: impl ToString,
        password: impl ToString,
    ) -> Self {
        self.config
            .authentication(AuthMethod::sql_server(username, password));
        self
    }

    /// Set [`tiberius::AuthMethod`] as authentication method.
    pub fn authentication(mut self, authentication: AuthMethod) -> Self {
        self.config.authentication(authentication);
        self
    }

    /// See [`tiberius::Config::trust_cert`]
    pub fn trust_cert(mut self) -> Self {
        self.config.trust_cert();
        self
    }

    /// Set [`tiberius::EncryptionLevel`] as enctryption method.
    pub fn encryption(mut self, encryption: EncryptionLevel) -> Self {
        self.config.encryption(encryption);
        self
    }

    /// See [`tiberius::Config::trust_cert_ca`]
    pub fn trust_cert_ca(mut self, path: impl ToString) -> Self {
        self.config.trust_cert_ca(path);
        self
    }

    /// Instance name defined in `Sql Browser`, defaults to None.
    pub fn instance_name(mut self, name: impl ToString) -> Self {
        self.config.instance_name(name);
        self
    }

    /// See [`tiberius::Config::application_name`]
    pub fn application_name(mut self, name: impl ToString) -> Self {
        self.config.application_name(name);
        self
    }

    /// Set pool size, defaults to 10.
    pub fn max_size(mut self, value: usize) -> Self {
        self.pool_config.max_size = value;
        self
    }

    /// Set timeout for when waiting for a connection object to become available.
    pub fn wait_timeout<T: Into<f64> + Copy>(mut self, value: T) -> Self {
        self.pool_config.timeouts.wait = Some(Duration::from_secs_f64(value.into()));
        self.set_runtime(Runtime::Tokio1);
        self
    }

    /// Set timeout for when creating a new connection object.
    pub fn create_timeout<T: Into<f64> + Copy>(mut self, value: T) -> Self {
        self.pool_config.timeouts.create = Some(Duration::from_secs_f64(value.into()));
        self.set_runtime(Runtime::Tokio1);
        self
    }

    /// Set timeout for when recycling a connection object.
    pub fn recycle_timeout<T: Into<f64> + Copy>(mut self, value: T) -> Self {
        self.pool_config.timeouts.recycle = Some(Duration::from_secs_f64(value.into()));
        self.set_runtime(Runtime::Tokio1);
        self
    }

    /// Attach a `sync fn` as hook to connection pool.
    /// The hook will be called each time before a connection [`deadpool::managed::Object`] is recycled.
    pub fn pre_recycle_sync<T>(mut self, hook: T) -> Self
    where
        T: Fn(&mut Client, &Metrics) -> HookResult<Error> + Sync + Send + 'static,
    {
        self.hooks.pre_recycle.push(Hook::sync_fn(hook));
        self
    }

    /// Attach an `async fn` as hook to connection pool.
    /// The hook will be called each time before a connection [`deadpool::managed::Object`] is recycled.
    pub fn pre_recycle_async<T>(mut self, hook: T) -> Self
    where
        T: for<'a> Fn(&'a mut Client, &'a Metrics) -> HookFuture<'a, Error> + Sync + Send + 'static,
    {
        self.hooks.pre_recycle.push(Hook::async_fn(hook));
        self
    }

    /// Attach a `sync fn` as hook to connection pool.
    /// The hook will be called each time af after a connection [`deadpool::managed::Object`] is recycled.
    pub fn post_recycle_sync<T>(mut self, hook: T) -> Self
    where
        T: Fn(&mut Client, &Metrics) -> HookResult<Error> + Sync + Send + 'static,
    {
        self.hooks.post_recycle.push(Hook::sync_fn(hook));
        self
    }

    /// Attach an `async fn` as hook to connection pool.
    /// The hook will be called each time after a connection [`deadpool::managed::Object`] is recycled.
    pub fn post_recycle_async<T>(mut self, hook: T) -> Self
    where
        T: for<'a> Fn(&'a mut Client, &'a Metrics) -> HookFuture<'a, Error> + Sync + Send + 'static,
    {
        self.hooks.post_recycle.push(Hook::async_fn(hook));
        self
    }

    /// Attach a `sync fn` as hook to connection pool.
    /// The hook will be called each time after a connection [`deadpool::managed::Object`] is created.
    pub fn post_create_sync<T>(mut self, hook: T) -> Self
    where
        T: Fn(&mut Client, &Metrics) -> HookResult<Error> + Sync + Send + 'static,
    {
        self.hooks.post_create.push(Hook::sync_fn(hook));
        self
    }

    /// Attach an `async fn` as hook to connection pool.
    /// The hook will be called each time after a connection [`deadpool::managed::Object`] is created.
    pub fn post_create_async<T>(mut self, hook: T) -> Self
    where
        T: for<'a> Fn(&'a mut Client, &'a Metrics) -> HookFuture<'a, Error> + Sync + Send + 'static,
    {
        self.hooks.post_create.push(Hook::async_fn(hook));
        self
    }

    fn set_runtime(&mut self, value: Runtime) {
        self.runtime = Some(value);
    }
}

struct Hooks {
    pre_recycle: Vec<Hook<Manager>>,
    post_recycle: Vec<Hook<Manager>>,
    post_create: Vec<Hook<Manager>>,
}

impl Default for Hooks {
    fn default() -> Self {
        Hooks {
            pre_recycle: Vec::<Hook<Manager>>::new(),
            post_recycle: Vec::<Hook<Manager>>::new(),
            post_create: Vec::<Hook<Manager>>::new(),
        }
    }
}
