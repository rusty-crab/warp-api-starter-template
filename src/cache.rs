use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};

pub async fn get_or_create<'a, K, T, F, P>(
    con: &mut MultiplexedConnection,
    key: K,
    create_fn: F,
) -> anyhow::Result<T>
where
    K: redis::ToRedisArgs + Clone + Send + Sync + 'a,
    T: Serialize + DeserializeOwned,
    F: Fn() -> P,
    P: core::future::Future<Output = anyhow::Result<(T, usize)>>,
{
    if let Ok(item) = get(con, key.clone()).await {
        Ok(item)
    } else {
        let (item, expiry) = create_fn().await?;
        set_ex(con, key, &item, expiry).await?;
        Ok(item)
    }
}

pub async fn get<'a, K, T>(con: &mut MultiplexedConnection, key: K) -> anyhow::Result<T>
where
    K: redis::ToRedisArgs + Send + Sync + 'a,
    T: DeserializeOwned,
{
    let bytes: Vec<u8> = con.get(key).await?;
    Ok(bincode::deserialize(&bytes)?)
}

pub async fn set_ex<'a, K, T>(
    con: &mut MultiplexedConnection,
    key: K,
    value: &T,
    seconds: usize,
) -> anyhow::Result<()>
where
    K: redis::ToRedisArgs + Send + Sync + 'a,
    T: Serialize,
{
    con.set_ex(key, bincode::serialize(value)?, seconds).await?;

    Ok(())
}
