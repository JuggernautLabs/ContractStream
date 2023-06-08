// logout
// // request_cover_letter
// input_cover_letter
// pending_job_actions = (reject, proposal)
// request_proposal(job_id)

use actix_cors::Cors;
use actix_web::{dev::Service, http::header}; // Add this line
use actix_multipart::Multipart;
//use tokio_stream::stream_ext::StreamExt;
use futures::{StreamExt, TryStreamExt};
use anyhow::anyhow;
use futures::future::try_join_all;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};
use ts_rs::TS;

use actix_web::{
    cookie::{time::Duration, Cookie},
    delete, get,
    middleware::Logger,
    post,
    web::Path,
    web::{self, Data, Form, Json},
    App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError,
};
use serde::{Deserialize, Serialize};

use thiserror::Error;
use uuid::Uuid;

use crate::db_utils::FetchId;
use crate::{
    db::{Database, Job, Resume, SearchContext, VerifiedUser},
    db_utils::Id,
};

static PY_URL: &str = "http://0.0.0.0:8081";

#[derive(PartialEq)]
struct LoginCookie {
    cookie_id: Uuid,
    death_date: Instant,
    user: VerifiedUser,
}

impl LoginCookie {
    pub fn new(user: VerifiedUser, ttl: Duration) -> Self {
        let uuid = Uuid::new_v4();
        let death_date = Instant::now() + ttl;
        Self {
            cookie_id: uuid,
            death_date,
            user,
        }
    }
}

impl<'a> Into<Cookie<'a>> for &LoginCookie {
    fn into(self) -> Cookie<'a> {
        Cookie::build("session_id", self.cookie_id.to_string()).finish()
    }
}

struct AppState {
    database: Database,
    login_cache: Mutex<BTreeMap<String, Arc<LoginCookie>>>,
}

impl AppState {
    pub fn verify_user(&self, req: HttpRequest) -> Result<Arc<LoginCookie>, AppError> {
        let cookie = req.cookie("session_id").ok_or(AppError::InvalidSession)?;
        let session_id = cookie.value();
        let login_cache = self.login_cache.lock().unwrap();
        let res: Result<Arc<LoginCookie>, AppError> = login_cache
            .get(session_id)
            .cloned()
            .ok_or(AppError::InvalidSession);

        res
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("login failed")]
    LoginError(anyhow::Error),
    #[error("signup failed")]
    SignupError(anyhow::Error),
    #[error("user not found")]
    UserNotFound,
    #[error("database error {0}")]
    DatabaseError(#[from] anyhow::Error),
    #[error("invalid session")]
    InvalidSession,
    #[error("input deserialization failed `{0}`")]
    InvalidShape(String),
    #[error("Internal error `{0}`")]
    InternalError(anyhow::Error),
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::LoginError(_) => StatusCode::UNAUTHORIZED,
            AppError::SignupError(_) => StatusCode::BAD_REQUEST,
            AppError::UserNotFound => StatusCode::NOT_FOUND,
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InvalidSession => StatusCode::UNAUTHORIZED,
            AppError::InvalidShape(_) => StatusCode::BAD_REQUEST,
            AppError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[post("/login")]
async fn login(
    _req: HttpRequest,
    login_form: Json<LoginForm>,
    data: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let user = data
        .database
        .get_user(login_form.username.clone(), login_form.password.clone())
        .await
        .map_err(AppError::LoginError)?;
    let session_cookie = LoginCookie::new(user, Duration::hours(1));
    let mut res = HttpResponse::Ok()
        .append_header(("credentials", "include"))
        .body("login successful".to_string());

    res.add_cookie(&(&session_cookie).into()).unwrap();

    data.login_cache.lock().unwrap().insert(
        session_cookie.cookie_id.to_string(),
        Arc::new(session_cookie),
    );

    Ok(res)
}

#[post("/signup")]
async fn signup(
    login_form: Json<LoginForm>,
    data: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let user = data
        .database
        .add_user(login_form.username.clone(), login_form.password.clone())
        .await
        .map_err(AppError::SignupError)?;
    Ok(HttpResponse::Ok().body(format!("{:?}", user)))
}

#[get("/check_login")]
async fn check_login(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;
    Ok(HttpResponse::Ok())
}

#[get("/pending_jobs")]
async fn pending_jobs(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;
    let database = &state.database;
    let user = &login_cookie.user;

    let pending_jobs = database
        .get_user_pending_jobs(user)
        .await
        .map_err(AppError::DatabaseError)?;

    Ok(HttpResponse::Ok().body(serde_json::to_string(&pending_jobs).unwrap()))
}

use reqwest::{Client, StatusCode};

#[derive(Deserialize)]
struct ClassifyResponse {
    classification: i32,
}

#[derive(Deserialize)]
struct ProposalResponse {
    proposal: String,
}

#[get("/next_pending_job")]
async fn next_pending_job(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;
    let database = &state.database;
    let user = &login_cookie.user;
    let pending_jobs1 = database
        .get_user_pending_jobs(user)
        .await
        .map_err(|e| AppError::DatabaseError(e))?;

    let client = Client::new();
    for job in pending_jobs1 {
        let res = client
            .get(format!("{}/classify_job", PY_URL))
            .query(&[("job_id", job.job_id), ("user_id", user.0.user_id)])
            .send()
            .await
            .map_err(|e| AppError::InternalError(e.into()))?;

        let res_json: ClassifyResponse = res
            .json()
            .await
            .map_err(|e| AppError::InternalError(e.into()))?;

        // -1 means no class and 1 means acceptable
        if res_json.classification != 0 {
            return Ok(web::Json(job));
        }
    }

    Err(AppError::InternalError(anyhow!("No pending jobs")))
}

#[post("/scrape_for_user")]
async fn scrape_for_user(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req.clone())?;
    let user = &login_cookie.user;

    let client = Client::new();

    client
        .post(format!("{}/scrape_for_user", PY_URL))
        .query(&[("user_id", user.0.user_id)])
        .send()
        .await
        .map_err(|e| AppError::InternalError(e.into()))?;

    Ok("")
}

// this needs to validate that a given job has been assigned to a particular user
// or we say screw it, generate a job for any job you want
// it's their money after all
#[get("/generate_proposal")]
async fn generate_proposal(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req.clone())?;
    let user = &login_cookie.user;
    use actix_web::web;
    let params = web::Query::<JobIdParam>::from_query(req.query_string())
        .map_err(|_| AppError::InvalidShape("No field 'job_id' in query".to_string()))?;
    let job_id = params.job_id.clone();

    let client = Client::new();

    let res = client
        .get(format!("{}/generate_proposal", PY_URL))
        .query(&[("job_id", job_id), ("user_id", user.0.user_id.to_string())])
        .send()
        .await
        .map_err(|e| AppError::InternalError(e.into()))?;

    let res_json: ProposalResponse = res
        .json()
        .await
        .map_err(|e| AppError::InternalError(e.into()))?;

    return Ok(web::Json(res_json.proposal));
}

#[derive(Deserialize)]
pub struct JobIdParam {
    job_id: String,
}
#[post("/accept_job")]
async fn accept_job(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req.clone())?;
    let user = &login_cookie.user;
    let db = &state.database;
    let params = web::Query::<JobIdParam>::from_query(req.query_string())
        .map_err(|_| AppError::InvalidShape("No field 'job_id' in query".to_string()))?;
    let jobid_param = params
        .job_id
        .parse::<i32>()
        .map_err(|e| AppError::InternalError(e.into()))?;
    let job = Job::fetch_id(&jobid_param, db.pool.clone()).await?;

    db.accept_pending_job(user, job.job_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.into()))?;

    return Ok("");
}

#[post("/reject_job")]
async fn reject_job(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req.clone())?;
    let user = &login_cookie.user;
    let db = &state.database;
    let params = web::Query::<JobIdParam>::from_query(req.query_string())
        .map_err(|_| AppError::InvalidShape("No field 'job_id' in query".to_string()))?;
    let jobid_param = params
        .job_id
        .parse::<i32>()
        .map_err(|e| AppError::InternalError(e.into()))?;
    let job = Job::fetch_id(&jobid_param, db.pool.clone()).await?;

    db.reject_pending_job(user, job.job_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.into()))?;

    return Ok("");
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[ts(export)]
struct SearchContextReq {
    keywords: Vec<String>,
}

#[post("/search_context")]
async fn post_search_context(
    req: HttpRequest,
    context: Json<SearchContextReq>,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;

    let user = &login_cookie.user;
    let context = context.into_inner();

    let database = &state.database;
    let search_context = database
        .insert_search_context(user, context.keywords)
        .await
        .map_err(AppError::DatabaseError)?;

    let res = SearchContextRes::try_from_search_context(search_context, database).await?;
    Ok(HttpResponse::Ok().body(serde_json::to_string(&res).unwrap()))
}

#[derive(Serialize, TS)]
#[ts(export)]
struct SearchContextRes {
    #[serde(flatten)]
    search_context: SearchContextReq,
    context_id: <SearchContext as FetchId>::Id,
}

impl SearchContextRes {
    async fn try_from_search_context(
        context: SearchContext,
        database: &Database,
    ) -> Result<Self, AppError> {
        let search_context = SearchContextReq {
            keywords: context.keywords.clone(),
        };

        Ok::<_, AppError>(SearchContextRes {
            search_context,
            context_id: context.context_id,
        })
    }
}
#[get("/search_context")]
async fn get_search_context(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    log::debug!("get_search_context");
    let login_cookie = state.verify_user(req)?;
    let user = &login_cookie.user;
    let database = &state.database;

    let contexts = database
        .get_search_contexts_by_user(user)
        .await
        .map_err(AppError::DatabaseError)?;

    let future_reqs = contexts
        .iter()
        .map(|context| SearchContextRes::try_from_search_context(context.clone(), database));

    let search_contexts = try_join_all(future_reqs).await?;
    let json_string = serde_json::to_string(&search_contexts).unwrap();
    log::debug!("{json_string}");
    Ok(HttpResponse::Ok().body(json_string))
}

#[post("/upload_resume")]
async fn upload_resume(
    req: HttpRequest,
    mut payload: Multipart,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;
    let user = &login_cookie.user;
    let db = &state.database;

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field
            .content_disposition();

        let filename = content_disposition
            .get_filename()
            .ok_or_else(|| AppError::InternalError(actix_web::error::ParseError::Incomplete.into()))?;

        if filename.ends_with(".pdf") {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let data_chunk = chunk.map_err(|e| AppError::InternalError(anyhow!("Failed to parse pdf file".to_string())))?;
                data.extend_from_slice(&data_chunk);
            }

            let text = pdf_extract::extract_text_from_mem(&data)
                .map_err(|e| AppError::InternalError(e.into()))?;

            db.save_resume(user, Some(data), text).await?;
        }
    }
    Ok("")
}

#[delete("/search_context")]
async fn delete_search_context(
    req: HttpRequest,
    context_id: Json<i32>,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    log::debug!("delete");
    let login_cookie = state.verify_user(req)?;

    let user = &login_cookie.user;

    let database = &state.database;

    let _search_context = database
        .remove_search_context(user, context_id.into_inner())
        .await
        .map_err(|e| AppError::DatabaseError(e))?;

    Ok(HttpResponse::Ok())
}

// #[get("/filtered_jobs")]
// async fn filtered_jobs(
//     req: HttpRequest,
//     state: Data<Arc<AppState>>,
// ) -> Result<impl Responder, AppError> {
//     let login_cookie = state.verify_user(req)?;
//     let user = &login_cookie.user;

//     let database = &state.database;

//     todo!()

//     // Ok(HttpResponse::Ok().body(serde_json::to_string(&search_contexts).unwrap()))
// }

// #[get("create_search")]

pub async fn serve(addr: (&str, u16), database: Database) -> Result<(), anyhow::Error> {
    let app_data = AppState {
        database,
        login_cache: Mutex::new(BTreeMap::new()),
    };
    let app_data = Arc::new(app_data);

    std::env::set_var("RUST_LOG", "info,debug,actix_web=info,debug");
    env_logger::init();

    HttpServer::new(move || {
        let app = App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(app_data.clone()))
            .service(login)
            .service(check_login)
            .service(signup)
            .service(pending_jobs)
            .service(next_pending_job)
            .service(generate_proposal)
            .service(accept_job)
            .service(reject_job)
            .service(post_search_context)
            .service(get_search_context)
            .service(scrape_for_user)
            // .service(active_searches)
            .service(delete_search_context)
            .wrap(Logger::new("%a %{User-Agent}i"));

        app
    })
    .bind(addr)?
    .run()
    .await?;
    Ok(())
}

// TODO: don't drop tables for testing, super risky
#[cfg(test)]
mod tests {
    use crate::db::Database;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    async fn db() -> Result<Database, anyhow::Error> {
        /*
        let database_url =
            env::var("DATABASE_URL")
            .map_err(|_err| anyhow::anyhow!("Please specify database url"))?;
        */
        let database_url = "postgres://super:isGod@localhost:5432/auto_contractor".to_string();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;
        let database = Database::new(pool);
        Ok(database)
    }

    #[tokio::test]
    async fn accept_job() {
        let db = db().await.unwrap();
        db.drop_non_user_tables().await.unwrap();
        db.create_tables().await.unwrap();
        let username = "Jay".to_string();
        let user = db
            .get_user(username.clone(), "isPleb".to_string())
            .await
            .unwrap();

        let job = db
            .add_job(
                "title".to_string(),
                "website".to_string(),
                "description".to_string(),
                Some(1.into()),
                Some(1.into()),
                "post_url".to_string(),
                None,
            )
            .await
            .unwrap();

        db.add_decided_job(&user, job.job_id, true).await.unwrap();

        assert_eq!(
            db.get_user_accepted_jobs(&username).await.unwrap(),
            vec![job],
        );
    }

    #[tokio::test]
    async fn reject_job() {
        let db = db().await.unwrap();
        db.drop_non_user_tables().await.unwrap();
        db.create_tables().await.unwrap();
        let username = "Jay".to_string();
        let user = db
            .get_user(username.clone(), "isPleb".to_string())
            .await
            .unwrap();

        let job = db
            .add_job(
                "title".to_string(),
                "website".to_string(),
                "description".to_string(),
                Some(1.into()),
                Some(1.into()),
                "post_url".to_string(),
                None,
            )
            .await
            .unwrap();

        db.add_decided_job(&user, job.job_id, false).await.unwrap();

        assert_eq!(
            db.get_user_rejected_jobs(&username).await.unwrap(),
            vec![job.clone()],
        );

        // TODO right now this doesnt test anything since there's no add_pending_job fn
        db.remove_pending_job(&user, job.job_id).await.unwrap();
        assert_eq!(db.get_user_pending_jobs(&user).await.unwrap(), vec![],);
    }
}
