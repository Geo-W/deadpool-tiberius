# Deadpool & Tiberius simple impl

This crate served as connector and re-exporter of tiberius && deadpool to make create sql server connection pool easier.   
If you are looking for sql server ORM, take a look at [ssql](https://github.com/Geo-W/ssql).  

For full documentation pls visit [doc.rs](https://docs.rs/deadpool-tiberius/latest/deadpool_tiberius/).

### Example, chaining configs from tiberius and configs from pooling
```rust
use deadpool_tiberius;

#[tokio::main]
async fn main() -> deadpool_tiberius::SqlServerResult<()> {
    let pool = deadpool_tiberius::Manager::new()
        .host("localhost") // default to localhost
        .port(1433) // default to 
        .basic_authentication("username", "password")
        //  or .authentication(tiberius::AuthMethod)
        .database("database1")
        .trust_cert()
        .max_size(10)
        .wait_timeout(1.52)  // in seconds, default to no timeout
        .pre_recycle_sync(|_client, _metrics| {
            // do sth with client object and pool metrics
            Ok(())
        })
        .create_pool()?;

    let mut conn = pool.get().await?;
    let rows = conn.simple_query("SELECT 1");

    // Or construct server connection config from ADO string.
    const CONN_STR: &str = "Driver={SQL Server};Integrated Security=True;\
                            Server=DESKTOP-TTTTTTT;Database=master;\
                            Trusted_Connection=yes;encrypt=DANGER_PLAINTEXT;";
    let _pool = deadpool_tiberius::Manager::from_ado_string(CONN_STR)
        .max_size(20)
        .create_pool()?;
}

```