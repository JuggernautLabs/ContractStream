use std::fmt::Debug;

// use crate::models::*;
use async_trait::async_trait;
use sqlx::{Pool, Postgres};

#[async_trait]
pub trait FetchId: Sized {
    type ERROR: Send + Sync + 'static + Into<anyhow::Error>;
    type Id: std::fmt::Debug + Clone;
    type Ok: Send + Sync + Debug + Clone;
    async fn fetch_id(id: &Self::Id, pool: Pool<Postgres>) -> Result<Self::Ok, Self::ERROR>;
}

#[derive(Debug, Clone)]
pub struct Index<Struct: FetchId>(<Struct as FetchId>::Id);

impl<Struct: FetchId + Clone> Index<Struct> {
    pub fn new(id: <Struct as FetchId>::Id) -> Self {
        Self(id)
    }
    pub async fn fetch(
        &self,
        pool: sqlx::Pool<Postgres>,
    ) -> Result<<Struct as FetchId>::Ok, anyhow::Error> {
        let k = Struct::fetch_id(&self.0, pool)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(k)
    }
    pub fn id(&self) -> &<Struct as FetchId>::Id {
        &self.0
    }
}
