#![allow(dead_code)]

use crate::db::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{types::BigDecimal, Pool, Postgres};

// #[derive(Debug, Builder)]
// #[builder(derive(Debug))]
// pub struct PendingJob {
//     job_id: IndexItem<JobBuilder>,
//     user_id: IndexItem<UserBuilder>,
//     proposal_id: IndexItem<ProposalBuilder>,
// }
#[derive(Debug, Clone)]
pub struct User {
    user_id: i32,
    pub username: String,
    password_digest: (),
}

#[async_trait]
impl FromDatabase<i32> for User {
    type ERROR = anyhow::Error;

    async fn build_from_index(id: &i32, pool: Pool<Postgres>) -> Result<User, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!("select user_id, username from users where user_id = $1", id)
            .fetch_one(&mut conn)
            .await?;
        let user = User {
            username: row.username,
            user_id: row.user_id,
            password_digest: (),
        };
        Ok(user)
    }
}

impl User {
    pub async fn add_user(
        username: String,
        password: String,
        pool: Pool<Postgres>,
    ) -> Result<Self, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let record = sqlx::query!(
            r" INSERT INTO Users (username, password_digest) VALUES ($1,crypt($2, gen_salt('bf'))) RETURNING user_id",
            username,
            password,
        ).fetch_one(&mut conn).await?;
        Ok(User {
            username: username,
            user_id: record.user_id,
            password_digest: (),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Proposal {
    proposal_id: i32,
    user_id: IndexItem<User, i32>,
    job_id: IndexItem<Job, i32>,
    proposal: Option<String>,
}

#[async_trait]
impl FromDatabase<i32> for Proposal {
    type ERROR = anyhow::Error;

    async fn build_from_index(id: &i32, pool: Pool<Postgres>) -> Result<Proposal, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!("select * from Proposals where proposal_id = $1", id,)
            .fetch_one(&mut conn)
            .await?;
        Ok(Proposal {
            proposal_id: row.proposal_id,
            user_id: row.user_id.into(),
            job_id: row.job_id.into(),
            proposal: row.proposal,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    job_id: i32,
    title: String,
    website: String,
    description: String,
    budget: BigDecimal,
    hourly: Option<BigDecimal>,
    post_url: String,
}

#[async_trait]
impl FromDatabase<i32> for Job {
    type ERROR = anyhow::Error;

    async fn build_from_index(id: &i32, pool: Pool<Postgres>) -> Result<Job, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!("select * from jobs where job_id = $1", id,)
            .fetch_one(&mut conn)
            .await?;
        // let user = UserBuilder::default()
        //     .user_id(row.user_id)
        //     .username(row.username)
        //     .build()?;
        // Ok(user)
        todo!()
    }
}

struct Database {
    pool: Pool<Postgres>,
}

// impl Database {
//     fn new(pool: Pool<Postgres>) -> Self {
//         Database { pool: pool }
//     }

//     async fn build_from_db(
//         &self,
//         username: String,
//         password: String,
//     ) -> Result<User, anyhow::Error> {
//         let mut conn: sqlx::pool::PoolConnection<Postgres> = self.pool.acquire().await?;
//         let record = sqlx::query!(
//             r"SELECT user_id FROM Users WHERE username = $1 AND password_digest = crypt($2, password_digest)",
//             username,
//             password,
//         )
//         .fetch_one(&mut conn)
//         .await
//         .map_err(|_| {
//             UserBuilderError::ValidationError(format!("Couldn't find user: {:?}", username))
//         })?;

//         // verify password

//         let verified_user = User {
//             user_id: record.user_id,
//             username,
//             password: (),
//         };

//         Ok(verified_user)
//     }

//     async fn add_job(&self) -> Result<Job, anyhow::Error> {
//         let mut conn: sqlx::pool::PoolConnection<Postgres> = pool.acquire().await?;
//         let job = self.build()?;
//         let record = sqlx::query!(
//             r"INSERT INTO Jobs
//         (title, website, description, budget, hourly, post_url)
//         VALUES ($1, $2, $3, $4, $5, $6) RETURNING job_id",
//             job.title.clone(),
//             job.website.clone(),
//             job.description.clone(),
//             job.budget.clone(),
//             job.hourly.clone(),
//             job.post_url.clone()
//         )
//         .fetch_one(&mut conn)
//         .await?;

//         Ok(Job {
//             job_id: record.job_id,
//             title: job.title,
//             website: job.website,
//             description: job.description,
//             budget: job.budget,
//             hourly: job.hourly,
//             post_url: job.post_url,
//         })
//     }
//     async fn get_user_denied_jobs(
//         &self,
//         username: &str,
//     ) -> Result<Vec<(Job, Proposal)>, anyhow::Error> {
//         let mut conn = self.pool.acquire().await?;

//         static query: &str = r#"
//         SELECT j.*, p.*
//         FROM Jobs j
//         JOIN DecidedJobs d ON j.job_id = d.job_id
//         JOIN Proposals p ON d.proposal_id = p.proposal_id
//         JOIN Users u ON d.user_id = u.user_id
//         WHERE u.username = $1 AND d.accepted = false;
//         "#;

//         let rows = sqlx::query(query)
//             .bind(username)
//             .fetch_all(&mut conn)
//             .await?;

//         let denied_jobs = rows
//             .into_iter()
//             .map(|row| {
//                 (
//                     Job {
//                         job_id: row.get(0),
//                         title: row.get(1),
//                         website: row.get(2),
//                         description: row.get(3),
//                         budget: row.get(4),
//                         hourly: row.get(5),
//                         post_url: row.get(6),
//                     },
//                     Proposal {
//                         proposal_id: row.get(7),
//                         user_id: row.get(8),
//                         job_id: row.get(9),
//                     },
//                 )
//             })
//             .collect();

//         Ok(denied_jobs)
//     }
// }
