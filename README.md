# Deadpool & Tiberius simple impl

> Just chaining configs from tiberius and configs from pooling.

```rust
use deadpool_tiberius;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = deadpool_tiberius::Manager::new()
        .host("localhost") // default to localhost
        .port(1433) // default to 1433
        .authentication(AuthMethod::sql_server("username", "password"))
        .database("database1")
        .trust_cert()
        .max_size(10)
        .wait_timeout(1.52)  // in seconds, default to no timeout
        .pre_recycle_sync(|client, metrics| {
            // do sth with client object and pool metrics
            Ok(())
        })
        .create_pool()?;
    
    let conn = pool.get().await?;
    let rows = conn.simple_query("SELECT 1");
}
```