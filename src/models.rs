#![allow(dead_code)]

use crate::db::*;
use anyhow::Context;
use async_trait::async_trait;

use crate::db::Index;
use sqlx::{types::BigDecimal, Pool, Postgres};
use typed_builder::TypedBuilder;
#[derive(Debug, Clone)]
pub struct User {
    user_id: i32,
    pub username: String,
    password_digest: (),
}

pub struct VerifiedUser(User);

impl VerifiedUser {
    async fn add_proposal(
        &self,
        job_id: <Job as FetchId>::Id,
        proposal_text: &str,
        pool: sqlx::Pool<Postgres>,
    ) -> Result<Proposal, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!(
            "INSERT INTO Proposals (user_id, job_id, proposal)
        VALUES ($1, $2, $3)
        RETURNING proposal_id",
            self.0.user_id,
            job_id,
            proposal_text
        )
        .fetch_one(&mut conn)
        .await?;
        Ok(Proposal {
            user_id: Index::new(self.0.user_id),
            job_id: Index::new(job_id),
            proposal_id: row.proposal_id,
            proposal: Some(proposal_text.into()),
        })
    }
}
#[async_trait]
impl FetchId for User {
    type ERROR = anyhow::Error;
    type Id = i32;
    type Ok = Self;

    async fn fetch_id(id: &i32, pool: Pool<Postgres>) -> Result<User, anyhow::Error> {
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

#[derive(Debug, Clone)]
pub struct Proposal {
    proposal_id: i32,
    user_id: Index<User>,
    job_id: Index<Job>,
    proposal: Option<String>,
}

#[async_trait]
impl FetchId for Proposal {
    type ERROR = anyhow::Error;
    type Id = i32;
    type Ok = Self;
    async fn fetch_id(id: &i32, pool: Pool<Postgres>) -> Result<Proposal, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!("select * from Proposals where proposal_id = $1", id,)
            .fetch_one(&mut conn)
            .await?;
        Ok(Proposal {
            proposal_id: row.proposal_id,
            user_id: Index::<User>::new(row.user_id),
            job_id: Index::<Job>::new(row.job_id),
            proposal: row.proposal,
        })
    }
}

#[derive(Debug, Clone, TypedBuilder, Default)]
pub struct Job {
    job_id: i32,
    title: String,
    website: String,
    description: String,
    budget: Option<BigDecimal>,
    hourly: Option<BigDecimal>,
    post_url: String,
}

#[async_trait]
impl FetchId for Job {
    type ERROR = anyhow::Error;
    type Id = i32;
    type Ok = Self;
    async fn fetch_id(id: &i32, pool: Pool<Postgres>) -> Result<Job, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!("select * from jobs where job_id = $1", id)
            .fetch_one(&mut conn)
            .await?;
        Ok(Job {
            job_id: row.job_id,
            title: row.title,
            website: row.website,
            description: row.description,
            budget: row.budget,
            hourly: row.hourly,
            post_url: row.post_url,
        })
    }
}

#[derive(Debug)]
enum Decided {
    Accepted,
    Denied,
}
#[derive(Debug)]
pub struct DecidedJob {
    job_id: Index<Job>,
    decided: Decided,
}
#[derive(Debug, Clone)]
pub struct PendingJobId {
    job: Index<Job>,
    user: Index<User>,
}
#[derive(Debug, Clone)]
pub struct PendingJob {
    job_id: Index<Job>,
    user_id: Index<User>,
    proposal_id: Index<Proposal>,
}

#[async_trait]
impl FetchId for PendingJob {
    type ERROR = anyhow::Error;
    type Id = PendingJobId;
    type Ok = Vec<Self>;

    async fn fetch_id(
        id: &Self::Id,
        pool: Pool<Postgres>,
    ) -> Result<Vec<PendingJob>, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let _records = sqlx::query!(
            "SELECT * FROM PendingJobs WHERE job_id = $1 AND user_id = $2",
            id.job.id(),
            id.user.id(),
        )
        .fetch_all(&mut conn)
        .await?
        .iter()
        .flat_map(|record| {
            Ok::<_, anyhow::Error>(PendingJob {
                job_id: Index::new(record.job_id),
                user_id: Index::new(record.user_id),
                proposal_id: Index::new(
                    record
                        .proposal_id
                        .context("found pending job without proposal")?,
                ),
            })
        })
        .collect::<Vec<_>>();
        todo!()
    }
}

// impl PendingJob {
// why would I want only the jobs?
//     async fn fetch_all(pool: Pool<Postgres>) -> Result<Vec<Job>, anyhow::Error> {
//         let mut conn = pool.acquire().await?;
//         let row = sqlx::query_as!(
//             Job,
//             "SELECT * FROM Jobs j WHERE j.job_id IN (SELECT DISTINCT job_id FROM PendingJobs)"
//         )
//         .fetch_all(&mut conn)
//         .await?;
//         todo!()
//     }
// }

struct Database {
    pool: Pool<Postgres>,
}

impl Database {
    fn new(pool: Pool<Postgres>) -> Self {
        Database { pool }
    }

    pub async fn add_user(
        &self,
        username: String,
        password: String,
    ) -> Result<VerifiedUser, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;
        let record = sqlx::query!(
            r" INSERT INTO Users (username, password_digest) VALUES ($1,crypt($2, gen_salt('bf'))) RETURNING user_id",
            username,
            password,
        ).fetch_one(&mut conn).await?;
        Ok(VerifiedUser(User {
            username,
            user_id: record.user_id,
            password_digest: (),
        }))
    }

    async fn get_user(
        &self,
        username: String,
        password: String,
    ) -> Result<VerifiedUser, anyhow::Error> {
        let mut conn: sqlx::pool::PoolConnection<Postgres> = self.pool.acquire().await?;
        let record = sqlx::query!(
            r"SELECT user_id FROM Users WHERE username = $1 AND password_digest = crypt($2, password_digest)",
            username,
            password,
        )
        .fetch_one(&mut conn)
        .await?;

        // verify password

        let verified_user = User {
            user_id: record.user_id,
            username,
            password_digest: (),
        };

        Ok(VerifiedUser(verified_user))
    }

    async fn add_job(
        &self,
        title: String,
        website: String,
        description: String,
        budget: Option<BigDecimal>,
        hourly: Option<BigDecimal>,
        post_url: String,
    ) -> Result<Job, anyhow::Error> {
        let mut conn: sqlx::pool::PoolConnection<Postgres> = self.pool.acquire().await?;

        let record = sqlx::query_as!(
            Job,
            r"INSERT INTO Jobs
        (title, website, description, budget, hourly, post_url)
        VALUES ($1, $2, $3, $4, $5, $6) RETURNING job_id,title,website,description,budget, hourly, post_url",
            title,
            website,
            description,
            budget,
            hourly,
            post_url
        )
        .fetch_one(&mut conn)
        .await?;

        Ok(record)
    }

    async fn get_user_denied_jobs(
        &self,
        username: &str,
    ) -> Result<Vec<(Job, Proposal)>, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        let rows = sqlx::query!(
            r#"
            SELECT j.*, p.proposal_id, p.user_id AS p_user_id, p.job_id AS p_job_id, p.proposal
            FROM Jobs j
            JOIN DecidedJobs d ON j.job_id = d.job_id
            JOIN Proposals p ON d.proposal_id = p.proposal_id
            JOIN Users u ON d.user_id = u.user_id
            WHERE u.username = $1 AND d.accepted = false;
        "#,
            username
        )
        .fetch_all(&mut conn)
        .await?;

        let denied_jobs = rows
            .into_iter()
            .map(|row| {
                (
                    Job {
                        job_id: row.job_id,
                        title: row.title,
                        website: row.website,
                        description: row.description,
                        budget: row.budget,
                        hourly: row.hourly,
                        post_url: row.post_url,
                    },
                    Proposal {
                        proposal_id: row.proposal_id,
                        user_id: Index::<User>::new(row.p_user_id),
                        job_id: Index::<Job>::new(row.p_job_id),
                        proposal: row.proposal,
                    },
                )
            })
            .collect();

        Ok(denied_jobs)
    }

    async fn get_user_pending_jobs(&self, username: &str) -> Result<Vec<Job>, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        let rows = sqlx::query_as!(
            Job,
            "
        SELECT j.*
        FROM Jobs j
        JOIN PendingJobs p ON j.job_id = p.job_id
        JOIN Users u ON p.user_id = u.user_id
        WHERE u.username = $1;
        ",
            username
        )
        .fetch_all(&mut conn)
        .await?;

        Ok(rows)
    }
    async fn get_user_accepted_jobs(
        &self,
        username: &str,
    ) -> Result<Vec<(Job, Proposal)>, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        let rows = sqlx::query!(
            r#"
        SELECT j.*, p.proposal_id, p.user_id AS p_user_id, p.job_id AS p_job_id, p.proposal
        FROM Jobs j
        JOIN DecidedJobs d ON j.job_id = d.job_id
        JOIN Proposals p ON d.proposal_id = p.proposal_id
        JOIN Users u ON d.user_id = u.user_id
        WHERE u.username = $1 AND d.accepted = true;
        "#,
            username,
        )
        .fetch_all(&mut conn)
        .await?;

        let accepted_jobs = rows
            .into_iter()
            .map(|row| {
                (
                    Job {
                        job_id: row.job_id,
                        title: row.title,
                        website: row.website,
                        description: row.description,
                        budget: row.budget,
                        hourly: row.hourly,
                        post_url: row.post_url,
                    },
                    Proposal {
                        proposal_id: row.proposal_id,
                        user_id: Index::<User>::new(row.p_user_id),
                        job_id: Index::<Job>::new(row.p_job_id),
                        proposal: row.proposal,
                    },
                )
            })
            .collect();

        Ok(accepted_jobs)
    }

    async fn remove_pending_job(&self, job_id: <Job as FetchId>::Id) -> Result<(), anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        let _rows = sqlx::query!(
            r#"
        DELETE FROM PendingJobs
        WHERE job_id = $1;
        "#,
            job_id
        )
        .fetch_all(&mut conn)
        .await?;

        Ok(())
    }

    async fn get_user_decided_jobs(
        &self,
        user_id: <User as FetchId>::Id,
    ) -> Result<Vec<DecidedJob>, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        let rows = sqlx::query!(
            r#"
            SELECT job_id, CASE
            WHEN accepted THEN 1
            ELSE 0
        END as accepted_int
        FROM DecidedJobs
        WHERE user_id = $1;
        "#,
            user_id,
        )
        .fetch_all(&mut conn)
        .await?;

        let decided_jobs = rows
            .into_iter()
            .map(|row| DecidedJob {
                job_id: Index::new(row.job_id),
                decided: match row.accepted_int {
                    Some(i) => match i {
                        0 => Decided::Denied,
                        1 => Decided::Accepted,
                        _ => unreachable!(
                            "got value other than 0 or 1 from either accepted or unaccepted job_id, value: {},{}",
                            row.job_id, i
                        ),
                    },
                    _ => unreachable!(
                        "got null from either accepted or unaccepted job_id: {}",
                        row.job_id
                    ),
                },
            })
            .collect();

        Ok(decided_jobs)
    }
}

// async fn get_all_pending_jobs(pool: &Pool<Postgres>, username: &str) -> Result<Vec<(Box<Dyn)>, anyhow::Error> {
//     let mut conn = pool.acquire().await?;
//     let query = "SELECT job_id, title FROM Jobs j WHERE j.job_id IN (SELECT DISTINCT job_id FROM PendingJobs);"

//     let rows = sqlx::query(query)
//         .bind(username)
//         .fetch_all(&mut conn)
//         .await?;

//     Ok(())
// }
