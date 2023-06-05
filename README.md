# Deadpool & Tiberius simple impl

> Just chaining configs from tiberius and create pool.

```rust
use deadpool_tiberius;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = deadpool_tiberius::Manager::new()
        .host("localhost")
        .authentication(AuthMethod::sql_server("test", "test"))
        .database("database1")
        .trust_cert()
        .create_pool()?;
    
    let conn = pool.get().await?;
    let rows = conn.simple_query("SELECT 1");
}
```