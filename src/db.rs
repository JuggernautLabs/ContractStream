use std::cell::RefCell;

// use crate::models::*;
use async_trait::async_trait;
use sqlx::{pool, postgres::PgRow, Pool, Postgres};

#[async_trait]
pub trait FromDatabase: Sized {
    type OK: Send + Sync;
    type ERROR: Send + Sync + 'static + Into<anyhow::Error>;
    type Id: Send + Sync + Into<IndexItem<Self>>;

    fn set_id(id: Self::Id) -> Self;
    fn id(&self) -> Self::Id;

    async fn build_from_index(&self, pool: Pool<Postgres>) -> Result<Self::OK, Self::ERROR>;
    // async fn save_to_db(&self, pool: Pool<Postgres>) -> Result<Self::OK, Self::ERROR>;
}

pub enum Either<T, K> {
    Right(K),
    Left(T),
}
#[derive(Debug, Clone)]
pub enum IndexItem<T: FromDatabase, K = <T as FromDatabase>::OK> {
    Builder(T),
    Value(K),
}

impl<T: FromDatabase<OK = K> + Clone, K: Clone> IndexItem<T> {
    pub async fn fetch(
        &self,
        pool: sqlx::Pool<Postgres>,
    ) -> Result<IndexItem<T, K>, anyhow::Error> {
        let a: Result<IndexItem<T, K>, anyhow::Error> = match &self {
            IndexItem::Builder(builder) => {
                let k = builder
                    .build_from_index(pool)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(IndexItem::Value(k))
            }
            IndexItem::Value(v) => Ok(IndexItem::Value(v.clone())),
        };
        a
    }
}
