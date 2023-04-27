use std::cell::RefCell;

// use crate::models::*;
use async_trait::async_trait;
use sqlx::{pool, postgres::PgRow, Pool, Postgres};

#[async_trait]
pub trait FromDatabase<K>: Sized {
    type OK: Send + Sync;
    type ERROR: Send + Sync + 'static + Into<anyhow::Error>;

    fn set_id(id: K) -> Self;
    fn id(&self) -> K;

    async fn build_from_index(&self, pool: Pool<Postgres>) -> Result<Self::OK, Self::ERROR>;
    // async fn save_to_db(&self, pool: Pool<Postgres>) -> Result<Self::OK, Self::ERROR>;
}

impl<T: FromDatabase<J>, J> From<J> for IndexItem<T, J> {
    fn from(id: J) -> Self {
        IndexItem::Builder(T::set_id(id))
    }
}

pub enum Either<T, K> {
    Right(K),
    Left(T),
}
#[derive(Debug, Clone)]
pub enum IndexItem<T: FromDatabase<J>, J, K = <T as FromDatabase<J>>::OK> {
    Builder(T),
    Value(K),
    _Unreachable(std::marker::PhantomData<J>),
}

impl<T: FromDatabase<J, OK = K> + Clone, K: Clone, J> IndexItem<T, J> {
    pub async fn fetch(
        &self,
        pool: sqlx::Pool<Postgres>,
    ) -> Result<IndexItem<T, J, K>, anyhow::Error> {
        let a: Result<IndexItem<T, J, K>, anyhow::Error> = match &self {
            IndexItem::Builder(builder) => {
                let k = builder
                    .build_from_index(pool)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(IndexItem::Value(k))
            }
            IndexItem::Value(v) => Ok(IndexItem::Value(v.clone())),
            IndexItem::_Unreachable(_) => unreachable!(),
        };
        a
    }
}
