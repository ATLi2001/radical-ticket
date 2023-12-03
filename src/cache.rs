use serde::{Deserialize, Serialize};
use worker::{Cache, Headers, Response, Result};

pub struct CacheKV {
    cache: Cache,
}

const DEFAULT_KEY_URL: &str = "http://radicalcache/keys";

impl CacheKV {
    pub fn new() -> Self {
        Self {
            cache: Cache::default(),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<Response>> {
        Ok(self.cache
            .get(format!("{DEFAULT_KEY_URL}/{key}"), false)
            .await?
            .map(|resp| {
                let mut headers = Headers::new();
                headers.append("Content-Type", "application/json").unwrap();
                resp.with_headers(headers)
            })
        )
    }

    pub async fn put<T>(&self, key: &str, val: &T) -> Result<()>
    where
        for<'a> T: Serialize + Deserialize<'a>,
    {
        let mut cache_headers = Headers::new();
        cache_headers.append("Cache-Control", "max-age=1000")?;
        cache_headers.append("Cache-Control", "public")?;
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
