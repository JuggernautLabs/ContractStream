#![allow(dead_code)]

use crate::db_utils::*;
use anyhow::Context;
use async_trait::async_trait;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db_utils::Index;
use sqlx::{types::BigDecimal, Pool, Postgres};
use typed_builder::TypedBuilder;

#[derive(TS)]
#[ts(export)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub user_id: i32,
    pub username: String,
    //pub password_digest: String, }
}

// doesn't implement Clone on purpose,
// with Clone many instances of a single verified user can exist
// haven't thought through if this makes type-safe verification weaker
#[derive(Debug)]
/// Represents a user that has logged in
/// Handling such a user should be done with care
pub struct VerifiedUser(pub User);

impl VerifiedUser {
    fn id(&self) -> Id<User> {
        return self.id();
    }
}
impl PartialEq for VerifiedUser {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[async_trait]
impl FetchId for User {
    type Id = i32;

    async fn fetch_id(id: &i32, pool: Pool<Postgres>) -> Result<User, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!("select user_id, username from users where user_id = $1", id)
            .fetch_one(&mut conn)
            .await?;
        let user = User {
            username: row.username,
            user_id: row.user_id,
            //password_digest: row.password_digest.expect("User has no password"),
        };
        Ok(user)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]

pub struct Resume {
    pub resume_id: i32,
    pub user_id: Index<User>,
    pub resume_text: String,
}

#[async_trait]
impl FetchId for Resume {
    type Id = i32;

    async fn fetch_id(id: &Self::Id, pool: Pool<Postgres>) -> Result<Self, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!(
            "select * from Resumes where resume_id = $1 and not deleted",
            id
        )
        .fetch_one(&mut conn)
        .await?;
        Ok(Resume {
            resume_id: id.clone(),
            user_id: Index::new(row.user_id),
            resume_text: row.resume_text,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct SearchContext {
    pub context_id: i32,
    pub resume_id: Option<Index<Resume>>,
    pub keywords: Vec<String>,
    pub user_id: Index<User>,
}

#[async_trait]
impl FetchId for SearchContext {
    type Id = i32;

    async fn fetch_id(id: &i32, pool: Pool<Postgres>) -> Result<SearchContext, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!(
            "select * from SearchContexts where context_id = $1 and not deleted",
            id,
        )
        .fetch_one(&mut conn)
        .await?;
        Ok(SearchContext {
            context_id: row.context_id,
            resume_id: row.resume_id.map(|rid| Index::<Resume>::new(rid)),
            keywords: row.keywords,
            user_id: Index::<User>::new(row.user_id),
        })
    }
}

#[derive(Debug, Clone, TS)]
#[ts(export)]
pub struct Proposal {
    proposal_id: i32,
    user_id: Index<User>,
    job_id: Index<Job>,
    proposal: Option<String>,
}

#[async_trait]
impl FetchId for Proposal {
    type Id = i32;

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

#[derive(Debug, Clone, TypedBuilder, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Job {
    pub job_id: i32,
    title: String,
    website: String,
    description: String,
    budget: Option<BigDecimal>,
    hourly: Option<BigDecimal>,
    post_url: String,
    summary: Option<String>,
}

#[async_trait]
impl FetchId for Job {
    type Id = i32;

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
            summary: row.summary,
        })
    }
}

#[derive(Debug, TS)]
#[ts(export)]
enum Decided {
    Accepted,
    Denied,
}
#[derive(Debug, TS)]
#[ts(export)]
pub struct DecidedJob {
    job_id: Index<Job>,
    decided: Decided,
}
#[derive(Debug, Clone, TS)]
#[ts(export)]
pub struct PendingJob {
    job_id: Index<Job>,
    user_id: Index<User>,
    proposal_id: Index<Proposal>,
}

#[async_trait]
impl FetchId for PendingJob {
    type Id = (Id<Job>, Id<User>);

    async fn fetch_id(id: &Self::Id, pool: Pool<Postgres>) -> Result<PendingJob, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let record = sqlx::query!(
            "SELECT * FROM PendingJobs WHERE job_id = $1 AND user_id = $2",
            id.0,
            id.1,
        )
        .fetch_one(&mut conn)
        .await?;

        Ok::<_, anyhow::Error>(PendingJob {
            job_id: Index::new(record.job_id),
            user_id: Index::new(record.user_id),
            proposal_id: Index::new(
                record
                    .proposal_id
                    .context("found pending job without proposal")?,
            ),
        })
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

pub struct Database {
    pub pool: Pool<Postgres>,
}

impl Database {
    pub fn new(pool: Pool<Postgres>) -> Self {
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
            //password_digest: password,
        }))
    }

    pub async fn get_user(
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
            //password_digest: password,
        };

        Ok(VerifiedUser(verified_user))
    }

    pub async fn add_job(
        &self,
        title: String,
        website: String,
        description: String,
        budget: Option<BigDecimal>,
        hourly: Option<BigDecimal>,
        post_url: String,
        summary: Option<String>,
    ) -> Result<Job, anyhow::Error> {
        let mut conn: sqlx::pool::PoolConnection<Postgres> = self.pool.acquire().await?;

        let record = sqlx::query_as!(
            Job,
            r"INSERT INTO Jobs
        (title, website, description, budget, hourly, post_url, summary)
        VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING job_id,title,website,description,budget, hourly, post_url, summary",
            title,
            website,
            description,
            budget,
            hourly,
            post_url,
            summary,
        )
        .fetch_one(&mut conn)
        .await?;

        Ok(record)
    }
    pub async fn add_job_if_not_exists(
        &self,
        title: String,
        website: String,
        description: String,
        budget: Option<BigDecimal>,
        hourly: Option<BigDecimal>,
        post_url: String,
        summary: Option<String>,
    ) -> Result<Index<Job>, anyhow::Error> {
        let mut conn: sqlx::pool::PoolConnection<Postgres> = self.pool.acquire().await?;

        let record = sqlx::query!(
            r"WITH new_job AS (
                INSERT INTO Jobs (title, website, description, budget, hourly, post_url, summary)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (post_url) DO NOTHING
                RETURNING job_id
            )
            SELECT job_id
            FROM new_job
            UNION ALL
            SELECT job_id
            FROM Jobs
            WHERE post_url = $6
            LIMIT 1;",
            title,
            website,
            description,
            budget,
            hourly,
            post_url,
            summary,
        )
        .fetch_one(&mut conn)
        .await?;

        Ok(Index::new(
            record.job_id.context("job_id not returned, fatal error")?,
        ))
    }
    pub async fn add_decided_job(
        &self,
        user: &VerifiedUser,
        job_id: Id<Job>,
        accepted: bool,
    ) -> Result<(), anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query!(
            r"INSERT INTO DecidedJobs
            (user_id, job_id, accepted)
            VALUES ($1, $2, $3)",
            user.0.user_id,
            job_id,
            accepted,
        )
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn get_user_denied_jobs(
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
                        summary: row.summary,
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

    pub async fn get_user_pending_jobs(
        &self,
        user: &VerifiedUser,
    ) -> Result<Vec<Job>, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;
        let username = user.0.username.clone();
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
    pub async fn get_user_accepted_jobs(
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
                        summary: row.summary,
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
    pub async fn get_user_decided_jobs(
        &self,
        user_id: Id<User>,
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
    pub async fn remove_pending_job(&self, user: &VerifiedUser, job_id: Id<Job>) -> Result<(), anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        let _rows = sqlx::query!(
            r#"
        DELETE FROM PendingJobs
        WHERE job_id = $2 AND user_id = $1;
        "#,
            user.0.user_id,
            job_id,
        )
        .fetch_all(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn save_resume(
        &self,
        user: &VerifiedUser,
        resume_text: String,
    ) -> Result<Resume, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;
        let resume_id = sqlx::query!(
            "INSERT INTO Resumes (user_id, resume_text)
            VALUES ($1, $2)
            RETURNING resume_id
            ",
            user.id(),
            resume_text
        )
        .fetch_one(&mut conn)
        .await?
        .resume_id;

        Ok(Resume {
            resume_id,
            user_id: Index::new(user.id()),
            resume_text,
        })
    }

    pub async fn remove_resume(
        &self,
        user: &VerifiedUser,
        resume_id: Id<Resume>,
    ) -> Result<(), anyhow::Error> {
        let mut conn = self.pool.acquire().await?;
        let _resume_id = sqlx::query!(
            "UPDATE Resumes
            SET deleted = true
            WHERE user_id = $1
            AND resume_id = $2",
            user.id(),
            resume_id,
        )
        .fetch_one(&mut conn)
        .await?;

        Ok(())
    }

    pub async fn add_proposal(
        &self,
        user: &VerifiedUser,
        job_id: Id<Job>,
        proposal_text: &str,
        pool: &sqlx::Pool<Postgres>,
    ) -> Result<Proposal, anyhow::Error> {
        let mut conn = pool.acquire().await?;
        let row = sqlx::query!(
            "INSERT INTO Proposals (user_id, job_id, proposal)
        VALUES ($1, $2, $3)
        RETURNING proposal_id",
            user.id(),
            job_id,
            proposal_text
        )
        .fetch_one(&mut conn)
        .await?;
        Ok(Proposal {
            user_id: Index::new(user.id()),
            job_id: Index::new(job_id),
            proposal_id: row.proposal_id,
            proposal: Some(proposal_text.into()),
        })
    }
    pub async fn insert_search_context(
        &self,
        user: &VerifiedUser,
        resume_id: Id<Resume>,
        keywords: Vec<String>,
    ) -> Result<SearchContext, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;
        let keyword_arr = keywords.as_slice();

        let user_id = user.id();
        let record = sqlx::query!(
            "INSERT INTO SearchContexts (resume_id, keywords, user_id)
            VALUES ($1, $2, $3)
            RETURNING context_id",
            resume_id,
            keyword_arr,
            user_id
        )
        .fetch_one(&mut conn)
        .await?;

        Ok(SearchContext {
            context_id: record.context_id,
            resume_id: Some(Index::new(resume_id)),
            keywords,
            user_id: Index::new(resume_id),
        })
    }
    pub async fn remove_search_context(
        &self,
        user: &VerifiedUser,
        context_id: Id<SearchContext>,
    ) -> Result<(), anyhow::Error> {
        let mut conn = self.pool.acquire().await?;

        let user_id = user.id();
        let record = sqlx::query!(
            "UPDATE SearchContexts
            SET deleted = true
            WHERE context_id = $1
            AND user_id = $2
            ",
            context_id,
            user_id,
        )
        .fetch_one(&mut conn)
        .await?;

        Ok(())
    }
    pub async fn get_search_contexts_by_user(
        &self,
        user_id: &VerifiedUser,
    ) -> Result<Vec<SearchContext>, anyhow::Error> {
        let mut conn = self.pool.acquire().await?;
        let career_info_rows = sqlx::query!(
            "SELECT context_id, resume_id, user_id, keywords FROM SearchContexts WHERE user_id = $1",
            user_id.id()
        )
        .fetch_all(&mut conn)
        .await?;

        let result = career_info_rows
            .into_iter()
            .map(|row| {
                let resume_id = row.resume_id.map(|id| Index::<Resume>::new(id));
                SearchContext {
                    context_id: row.context_id,
                    resume_id,
                    keywords: row.keywords,
                    user_id: Index::new(row.user_id),
                }
            })
            .collect::<Vec<SearchContext>>();

        Ok(result)
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
