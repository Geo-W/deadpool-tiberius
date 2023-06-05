pub use deadpool;
use deadpool::{async_trait, managed};
use deadpool::managed::RecycleResult;

pub use tiberius;
use tiberius::{AuthMethod, EncryptionLevel};
use tokio_util::compat::TokioAsyncWriteCompatExt;

mod error;

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;
pub type Pool = managed::Pool<Manager>;


pub struct Manager {
    config: tiberius::Config,
    modify_tcp_stream:
    Box<dyn Fn(&tokio::net::TcpStream) -> tokio::io::Result<()> + Send + Sync + 'static>,
}

#[async_trait]
impl managed::Manager for Manager {
    type Type = Client;
    type Error = error::Error;

    async fn create(&self) -> Result<Client, Self::Error> {
        let tcp = tokio::net::TcpStream::connect(self.config.get_addr()).await?;
        (self.modify_tcp_stream)(&tcp)?;
        let client = Client::connect(self.config.clone(), tcp.compat_write()).await?;
        Ok(client)
    }
    async fn recycle(&self, obj: &mut Self::Type) -> RecycleResult<Self::Error> {
        Ok(())
    }
}


impl Manager {
    pub fn new() -> Self {
        Self {
            config: tiberius::Config::new(),
            modify_tcp_stream: Box::new(|tcp_stream| tcp_stream.set_nodelay(true)),
        }
    }

    pub fn create_pool(self) -> Result<Pool, error::Error> {
        let pool = Pool::builder(self).build().unwrap();
        Ok(pool)
    }

    // belows are just copies from tiberius config
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
}