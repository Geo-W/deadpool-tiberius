use std::mem::replace;
use std::time::Duration;

pub use deadpool;
use deadpool::{
    async_trait, managed,
    managed::{Hook, HookFuture, HookResult, Metrics, PoolConfig, RecycleError, RecycleResult},
    Runtime,
};
use tokio_util::compat::TokioAsyncWriteCompatExt;

pub use tiberius;
use tiberius::{AuthMethod, EncryptionLevel};

use crate::error::Error;

mod error;

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;
pub type Pool = managed::Pool<Manager>;

pub struct Manager {
    config: tiberius::Config,
    pool_config: PoolConfig,
    runtime: Option<Runtime>,
    hooks: Hooks,
    modify_tcp_stream:
        Box<dyn Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static>,
    enable_sql_browser: bool,
}

#[async_trait]
impl managed::Manager for Manager {
    type Type = Client;
    type Error = crate::error::Error;

    #[cfg(feature = "sql-browser")]
    async fn create(&self) -> Result<Client, Self::Error> {
        use tiberius::SqlBrowser;
        let tcp = if !self.enable_sql_browser {
            tokio::net::TcpStream::connect(self.config.get_addr()).await?
        } else {
            tokio::net::TcpStream::connect_named(&self.config).await?
        };
        (self.modify_tcp_stream)(&tcp)?;
        let client = Client::connect(self.config.clone(), tcp.compat_write()).await?;
        Ok(client)
    }

    #[cfg(not(feature = "sql-browser"))]
    async fn create(&self) -> Result<Client, Self::Error> {
        let tcp = tokio::net::TcpStream::connect(self.config.get_addr()).await?;
        (self.modify_tcp_stream)(&tcp)?;
        let client = Client::connect(self.config.clone(), tcp.compat_write()).await?;
        Ok(client)
    }

    async fn recycle(
        &self,
        conn: &mut Self::Type,
        // _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        match conn.simple_query("").await {
            Ok(_) => Ok(()),
            Err(e) => Err(RecycleError::Message(e.to_string())),
        }
    }
}

impl Manager {
    pub fn new() -> Self {
        Self {
            config: tiberius::Config::new(),
            pool_config: PoolConfig::default(),
            runtime: None,
            hooks: Hooks::default(),
            modify_tcp_stream: Box::new(|tcp_stream| tcp_stream.set_nodelay(true)),
            enable_sql_browser: false,
        }
    }

    pub fn create_pool(mut self) -> Result<Pool, error::Error> {
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

        Ok(pool.build().unwrap())
    }

    #[cfg(feature = "sql-browser")]
    pub fn enable_sql_browser(mut self) -> Self {
        self.enable_sql_browser = true;
        self
    }

    /// belows are tiberius config
    pub fn host(mut self, host: impl ToString) -> Self {
        self.config.host(host);
        self
    }

    pub fn database(mut self, database: impl ToString) -> Self {
        self.config.database(database);
        self
    }

    pub fn authentication(mut self, authentication: AuthMethod) -> Self {
        self.config.authentication(authentication);
        self
    }

    pub fn trust_cert(mut self) -> Self {
        self.config.trust_cert();
        self
    }

    pub fn encryption(mut self, encryption: EncryptionLevel) -> Self {
        self.config.encryption(encryption);
        self
    }

    pub fn trust_cert_ca(mut self, path: impl ToString) -> Self {
        self.config.trust_cert_ca(path);
        self
    }

    pub fn instance_name(mut self, name: impl ToString) -> Self {
        self.config.instance_name(name);
        self
    }

    pub fn application_name(mut self, name: impl ToString) -> Self {
        self.config.application_name(name);
        self
    }

    /// belows are pool config
    pub fn max_size(mut self, value: usize) -> Self {
        self.pool_config.max_size = value;
        self
    }

    pub fn wait_timeout<T: Into<f64> + Copy>(mut self, value: T) -> Self {
        self.pool_config.timeouts.wait = Some(Duration::from_secs_f64(value.into()));
        self.set_runtime(Runtime::Tokio1);
        self
    }

    pub fn create_timeout<T: Into<f64> + Copy>(mut self, value: T) -> Self {
        self.pool_config.timeouts.create = Some(Duration::from_secs_f64(value.into()));
        self.set_runtime(Runtime::Tokio1);
        self
    }

    pub fn recycle_timeout<T: Into<f64> + Copy>(mut self, value: T) -> Self {
        self.pool_config.timeouts.recycle = Some(Duration::from_secs_f64(value.into()));
        self.set_runtime(Runtime::Tokio1);
        self
    }

    pub fn pre_recycle_sync<T>(mut self, hook: T) -> Self
    where
        T: Fn(&mut Client, &Metrics) -> HookResult<Error> + Sync + Send + 'static,
    {
        self.hooks.pre_recycle.push(Hook::sync_fn(hook));
        self
    }

    pub fn pre_recycle_async<T>(mut self, hook: T) -> Self
    where
        T: for<'a> Fn(&'a mut Client, &'a Metrics) -> HookFuture<'a, Error> + Sync + Send + 'static,
    {
        self.hooks.pre_recycle.push(Hook::async_fn(hook));
        self
    }

    pub fn post_recycle_sync<T>(mut self, hook: T) -> Self
    where
        T: Fn(&mut Client, &Metrics) -> HookResult<Error> + Sync + Send + 'static,
    {
        self.hooks.post_recycle.push(Hook::sync_fn(hook));
        self
    }

    pub fn post_recycle_async<T>(mut self, hook: T) -> Self
    where
        T: for<'a> Fn(&'a mut Client, &'a Metrics) -> HookFuture<'a, Error> + Sync + Send + 'static,
    {
        self.hooks.post_recycle.push(Hook::async_fn(hook));
        self
    }

    pub fn post_create_sync<T>(mut self, hook: T) -> Self
    where
        T: Fn(&mut Client, &Metrics) -> HookResult<Error> + Sync + Send + 'static,
    {
        self.hooks.post_create.push(Hook::sync_fn(hook));
        self
    }

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
