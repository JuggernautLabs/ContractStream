use std::cell::RefCell;

// use crate::models::*;
use async_trait::async_trait;
use sqlx::{pool, postgres::PgRow, Pool, Postgres};

#[async_trait]
pub trait FromDatabase<K: Clone>: Sized {
    type ERROR: Send + Sync + 'static + Into<anyhow::Error>;

    async fn build_from_index(id: &K, pool: Pool<Postgres>) -> Result<Self, Self::ERROR>;
    // async fn save_to_db(&self, pool: Pool<Postgres>) -> Result<Self::OK, Self::ERROR>;
}

impl<Struct: FromDatabase<Id>, Id: Clone> From<Id> for IndexItem<Struct, Id> {
    fn from(id: Id) -> Self {
        IndexItem::Builder(id)
    }
}

#[derive(Debug, Clone)]
pub enum IndexItem<Struct: FromDatabase<Id>, Id: Clone> {
    Builder(Id),
    Value(Struct),
}

impl<Struct: FromDatabase<Id> + Clone, Id: Clone> IndexItem<Struct, Id> {
    pub async fn fetch(
        &self,
        pool: sqlx::Pool<Postgres>,
    ) -> Result<IndexItem<Struct, Id>, anyhow::Error> {
        let a: Result<IndexItem<Struct, Id>, anyhow::Error> = match &self {
            IndexItem::Builder(id) => {
                let k = Struct::build_from_index(id, pool)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(IndexItem::Value(k))
            }
            IndexItem::Value(v) => Ok(IndexItem::Value(v.clone())),
        };
        a
    }
}
