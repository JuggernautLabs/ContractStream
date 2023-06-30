// logout
// // request_cover_letter
// input_cover_letter
// pending_job_actions = (reject, proposal)
// request_proposal(job_id)

use std::str::FromStr;
use std::sync::Arc;

use actix_cors::Cors;
use actix_multipart::Multipart;
// Add this line
//use tokio_stream::stream_ext::StreamExt;
use anyhow::anyhow;

use futures::future::try_join_all;
use futures::{StreamExt, TryStreamExt};

use reqwest::header::{HeaderName, HeaderValue, SET_COOKIE};
use ts_rs::TS;

use actix_web::{
    cookie::Cookie,
    delete, get,
    middleware::Logger,
    post,
    web::{self, Data, Json},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use serde::{Deserialize, Serialize};

use crate::appstate::{AppError, AppState, HEADER_SET_SESSION};
use crate::db::{Database, Job, SearchContext};
use crate::db_utils::FetchId;

static PY_URL: &str = "http://localhost:8081";

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[post("/login")]
async fn login(
    _req: HttpRequest,
    login_form: Json<LoginForm>,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let user = state
        .database
        .get_user(login_form.username.clone(), login_form.password.clone())
        .await
        .map_err(AppError::LoginError)?;

    let mut res = HttpResponse::Ok()
        .append_header(("credentials", "include"))
        .body("login successful".to_string());

    let login_cookie = state.login(user).await?;
    let cookie = Cookie::build("session_id", login_cookie.cookie_id.to_string()).finish();
    let headers = res.headers_mut();
    headers.append(
        HeaderName::from_str(HEADER_SET_SESSION.into()).unwrap(),
        HeaderValue::from_str(&cookie.to_string()).unwrap(),
    );
    res.add_cookie(&cookie).unwrap();

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
    let _login_cookie = state.verify_user(req).await?;
    Ok(HttpResponse::Ok())
}

#[get("/pending_jobs")]
async fn pending_jobs(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req).await?;
    let database = &state.database;
    let user = &login_cookie.user;

    let pending_jobs = database
        .get_user_pending_jobs(user)
        .await
        .map_err(AppError::DatabaseError)?;

    Ok(HttpResponse::Ok().body(serde_json::to_string(&pending_jobs).unwrap()))
}

use reqwest::Client;

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
    let login_cookie = state.verify_user(req).await?;
    let database = &state.database;
    let user = &login_cookie.user;
    let pending_jobs1 = database
        .get_user_pending_jobs(user)
        .await
        .map_err(AppError::DatabaseError)?;

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
    let login_cookie = state.verify_user(req.clone()).await?;
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
    let login_cookie = state.verify_user(req.clone()).await?;
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

    Ok(web::Json(res_json.proposal))
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
    let login_cookie = state.verify_user(req.clone()).await?;
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

    Ok("")
}

#[post("/reject_job")]
async fn reject_job(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req.clone()).await?;
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

    Ok("")
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
    let login_cookie = state.verify_user(req).await?;

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
        _database: &Database,
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
    let login_cookie = state.verify_user(req).await?;
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
    let login_cookie = state.verify_user(req).await?;
    let user = &login_cookie.user;
    let db = &state.database;

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();

        let filename = content_disposition.get_filename().ok_or_else(|| {
            AppError::InternalError(actix_web::error::ParseError::Incomplete.into())
        })?;

        if filename.ends_with(".pdf") {
            let mut data = Vec::new();
            while let Some(chunk) = field.next().await {
                let data_chunk = chunk.map_err(|_e| {
                    AppError::InternalError(anyhow!("Failed to parse pdf file".to_string()))
                })?;
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
    let login_cookie = state.verify_user(req).await?;

    let user = &login_cookie.user;

    let database = &state.database;

    database
        .remove_search_context(user, context_id.into_inner())
        .await
        .map_err(AppError::DatabaseError)?;

    Ok(HttpResponse::Ok())
}

// #[get("/filtered_jobs")]
// async fn filtered_jobs(
//     req: HttpRequest,
//     state: Data<Arc<AppState>>,
// ) -> Result<impl Responder, AppError> {
//     let login_cookie = state.verify_user(req).await?;
//     let user = &login_cookie.user;

//     let database = &state.database;

//     todo!()

//     // Ok(HttpResponse::Ok().body(serde_json::to_string(&search_contexts).unwrap()))
// }

// #[get("create_search")]

pub async fn serve(addr: (&str, u16), database: Database) -> Result<(), anyhow::Error> {
    let app_data = AppState::new(database);
    let app_data = Arc::new(app_data);

    std::env::set_var("RUST_LOG", "log,info,debug,actix_web=info,debug,log");
    env_logger::init();

    HttpServer::new(move || {
        App::new()
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
            .wrap(Logger::new("%a %{User-Agent}i"))
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
