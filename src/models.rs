#![allow(dead_code)]
use crate::db::*;
use anyhow::Context;
use async_trait::async_trait;
use derive_builder::Builder;
use num::cast::FromPrimitive;
use sqlx::{postgres::PgRow, types::BigDecimal, Pool, Postgres, Row};
macro_rules! format_query {
    ($table: expr, $id_name: expr, ($returns: tt)) => {
        format!(
            "select {} from {} where {} = $1",
            $returns, $table, $id_name
        )
    };
    ($table: expr, $id_name: expr) => {
        format!("select * from {} where {} = $1", $table, $id_name)
    };
}
#[derive(Debug, Builder)]
pub struct Proposal {
    proposal_id: i32,
    user_id: IndexItem<UserBuilder>,
    job_id: IndexItem<JobBuilder>,
}

#[derive(Debug, Builder)]
pub struct PendingJob {
    job_id: IndexItem<JobBuilder>,
    user_id: IndexItem<UserBuilder>,
    proposal_id: IndexItem<ProposalBuilder>,
}
#[derive(Debug, Builder, Clone)]
#[builder(public)]
#[builder(derive(Debug))]
// #[builder(build_fn(validate = "Self::validate"))]
pub struct User {
    #[builder(field(type = "i32"), setter(strip_option))]
    user_id: i32,
    pub username: String,
    #[builder(
        field(type = "Option<String>", build = "()"),
        setter(strip_option, name = "password")
    )]
    password_digest: (),
}

#[derive(Debug, Builder, Clone)]
#[builder(derive(Debug), build_fn(name = "build_from_index"))]
pub struct Job {
    #[builder(field(type = "i32"), setter(strip_option))]
    job_id: i32,
    title: String,
    website: String,
    description: String,
    #[builder(setter(strip_option))]
    budget: BigDecimal,
    #[builder(
        setter(strip_option),
        field(type = "Option<BigDecimal>", build = "self.hourly.clone()")
    )]
    hourly: Option<BigDecimal>,
    post_url: String,
}

#[async_trait]
impl FromDatabase for ProposalBuilder {
    type OK = Proposal;
    type ERROR = anyhow::Error;

    fn id(&self) -> i32 {
        self.job_id
    }
    fn set_id(id: i32) -> Self {
        ProposalBuilder::default().job_id(id).clone()
    }

    async fn build_from_index(&self, pool: Pool<Postgres>) -> Result<Proposal, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query_as!(
            ProposalBuilder,
            "select * from ProposalBuilder where job_id = $1",
            self.job_id,
        )
        .fetch_one(&mut conn)
        .await?;
        todo!()
    }
}

#[async_trait]
impl FromDatabase for JobBuilder {
    type OK = Job;
    type ERROR = anyhow::Error;

    fn id(&self) -> i32 {
        self.job_id
    }
    fn set_id(id: i32) -> Self {
        JobBuilder::default().job_id(id).clone()
    }
    async fn build_from_index(&self, pool: Pool<Postgres>) -> Result<Job, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query_as!(
            JobBuilder,
            "select * from jobs where job_id = $1",
            self.job_id,
        )
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
#[async_trait]
impl FromDatabase for UserBuilder {
    type OK = User;
    type ERROR = anyhow::Error;
    async fn build_from_index(&self, pool: Pool<Postgres>) -> Result<User, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!(
            "select user_id, username from users where user_id = $1",
            self.user_id,
        )
        .fetch_one(&mut conn)
        .await?;
        let user = UserBuilder::default()
            .user_id(row.user_id)
            .username(row.username)
            .build()?;
        Ok(user)
    }
}
struct Database {
    pool: Pool<Postgres>,
}

// impl Database {
//     fn new(pool: Pool<Postgres>) -> Self {
//         Database { pool: pool }
//     }
//     async fn add_user(&self, username: String, password: String) -> Result<User, anyhow::Error> {
//         let mut conn = self.pool.acquire().await?;
//         let record = sqlx::query!(
//             r" INSERT INTO Users (username, password_digest) VALUES ($1,crypt($2, gen_salt('bf'))) RETURNING user_id",
//             username,
//             password,
//         ).fetch_one(&mut conn).await?;
//         Ok(User {
//             username: username,
//             user_id: record.user_id,
//             password: (),
//         })
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
