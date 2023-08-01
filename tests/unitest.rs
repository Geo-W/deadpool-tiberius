#[cfg(test)]
mod tests {
    use tokio;
    use deadpool_tiberius::SqlServerResult;
    use futures_lite::stream::StreamExt;

    #[tokio::test]
    async fn login() {
        async fn main() -> SqlServerResult<()> {
            let pool = deadpool_tiberius::Manager::new()
                .host("localhost") // default to localhost
                .port(1433) // default to 1433
                .basic_authentication("username", "password")
                .database("database")
                .trust_cert()
                .max_size(10)
                .wait_timeout(1.52)  // in seconds, default to no timeout
                .pre_recycle_sync(|_client, _metrics| {
                    // do sth with client object and pool metrics
                    Ok(())
                })
                .create_pool()?;

            let mut conn = pool.get().await?;
            let mut rows = conn.simple_query("SELECT * FROM my_table").await?;
            while let Some(v) = rows.try_next().await? {
                dbg!(&v);
            }
            Ok(())
        }
        main().await.unwrap();
    }
}