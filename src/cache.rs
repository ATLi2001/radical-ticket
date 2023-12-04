use serde::{Deserialize, Serialize};
use worker::{Cache, Headers, Response, Result};

pub struct CacheKV {
    cache: Cache,
}

const DEFAULT_KEY_URL: &str = "http://radicalcache/keys";
const DEFAULT_CACHE_NAME: &str = "RADICAL_SSR";

impl CacheKV {
    pub async fn new() -> Self {
        Self {
            cache: Cache::open(DEFAULT_CACHE_NAME.to_owned()).await,
        }
    }

    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        for<'a> T: Serialize + Deserialize<'a>,
    {
        match self
            .cache
            .get(format!("{DEFAULT_KEY_URL}/{key}"), false)
            .await?
        {
            Some(mut resp) => Ok(Some(resp.json::<T>().await?)),
            None => Ok(None),
        }
    }

    pub async fn put<T>(&self, key: &str, val: &T) -> Result<()>
    where
        for<'a> T: Serialize + Deserialize<'a>,
    {
        let mut cache_headers = Headers::new();
        cache_headers.append("Cache-Control", "max-age=1000")?;
        cache_headers.append("Cache-Control", "public")?;
        cache_headers
            .append("Content-Type", "application/json")
            .unwrap();
        let cache_resp = Response::from_json::<T>(val)?.with_headers(cache_headers);
        self.cache
            .put(format!("{DEFAULT_KEY_URL}/{key}"), cache_resp)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        self.cache
            .delete(format!("{DEFAULT_KEY_URL}/{key}"), false)
            .await?;
        Ok(())
    }
}
