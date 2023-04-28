use std::cell::RefCell;

// use crate::models::*;
use async_trait::async_trait;
use sqlx::{pool, postgres::PgRow, Pool, Postgres};

#[async_trait]
pub trait FromDatabase: Sized {
    type ERROR: Send + Sync + 'static + Into<anyhow::Error>;
    type Id: std::fmt::Debug + Clone;

    async fn build_from_index(id: &Self::Id, pool: Pool<Postgres>) -> Result<Self, Self::ERROR>;
    // async fn save_to_db(&self, pool: Pool<Postgres>) -> Result<Self::OK, Self::ERROR>;
}

#[derive(Debug, Clone)]
pub enum IndexItem<Struct: FromDatabase> {
    Id(<Struct as FromDatabase>::Id),
    Value(Struct),
}

impl<Struct: FromDatabase + Clone> IndexItem<Struct> {
    pub async fn fetch(
        &self,
        pool: sqlx::Pool<Postgres>,
    ) -> Result<IndexItem<Struct>, anyhow::Error> {
        let a: Result<IndexItem<Struct>, anyhow::Error> = match &self {
            IndexItem::Id(id) => {
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
