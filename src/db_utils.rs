use std::fmt::Debug;

// use crate::models::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use ts_rs::TS;

#[async_trait]
pub trait FetchId: Sized {
    type Id: std::fmt::Debug + Clone + Serialize + for<'de> Deserialize<'de> + TS;
    async fn fetch_id(id: &Self::Id, pool: Pool<Postgres>) -> Result<Self, anyhow::Error>;
}

pub type Id<T> = <T as FetchId>::Id;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Index<Struct: FetchId>(<Struct as FetchId>::Id);

impl<Struct: FetchId + Clone> Index<Struct> {
    pub fn new(id: <Struct as FetchId>::Id) -> Self {
        Self(id)
    }
    pub async fn fetch(&self, pool: sqlx::Pool<Postgres>) -> Result<Struct, anyhow::Error> {
        let k = Struct::fetch_id(&self.0, pool)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(k)
    }
    pub fn id(&self) -> <Struct as FetchId>::Id {
        self.0.clone()
    }
}
