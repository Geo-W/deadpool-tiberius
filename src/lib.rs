pub use deadpool;
use deadpool::{
    async_trait,
    managed,
    managed::{Metrics, PoolConfig, RecycleResult},
    Runtime
};
use std::time::Duration;

pub use tiberius;
use tiberius::{AuthMethod, EncryptionLevel};
use tokio_util::compat::TokioAsyncWriteCompatExt;

mod error;

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;
pub type Pool = managed::Pool<Manager>;

pub struct Manager {
    config: tiberius::Config,
    pool_config: PoolConfig,
    runtime: Option<Runtime>,
    modify_tcp_stream:
        Box<dyn Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static>,
    enable_sql_browser: bool
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
        _obj: &mut Self::Type,
        // _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        Ok(())
    }
}

impl Manager {
    pub fn new() -> Self {
        Self {
            config: tiberius::Config::new(),
            pool_config: PoolConfig::default(),
            runtime: None,
            modify_tcp_stream: Box::new(|tcp_stream| tcp_stream.set_nodelay(true)),
            enable_sql_browser: false
        }
    }

    pub fn create_pool(self) -> Result<Pool, error::Error> {
        let config = self.pool_config;
        let runtime = self.runtime;
        let mut pool = Pool::builder(self).config(config);
        if let Some(v) = runtime {
            pool = pool.runtime(v);
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

    fn set_runtime(&mut self, value: Runtime) {
        self.runtime = Some(value);
    }
}
