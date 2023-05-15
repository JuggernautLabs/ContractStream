// logout
// // request_cover_letter
// input_cover_letter
// pending_job_actions = (reject, proposal)
// request_proposal(job_id)

use anyhow::anyhow;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use actix_web::{
    cookie::{time::Duration, Cookie},
    delete, get,
    middleware::Logger,
    post,
    web::Path,
    web::{self, Data, Form, Json},
    App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError,
};
use env_logger::Env;
use serde::Deserialize;

use thiserror::Error;
use uuid::Uuid;

use crate::db::{Database, VerifiedUser};

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
    #[error("database error")]
    DatabaseError(#[from] anyhow::Error),
    #[error("invalid session")]
    InvalidSession,
    #[error("input deserialization failed `{0}`")]
    InvalidShape(String),
    #[error("Internal error `{0}`")]
    InternalError(anyhow::Error),
}

impl ResponseError for AppError {}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}
#[post("/login")]
async fn login(
    _req: HttpRequest,
    login_form: Form<LoginForm>,
    data: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let user = data
        .database
        .get_user(login_form.username.clone(), login_form.password.clone())
        .await
        .map_err(AppError::LoginError)?;
    let session_cookie = LoginCookie::new(user, Duration::hours(1));
    let mut res = HttpResponse::Ok().body("login successful".to_string());

    res.add_cookie(&(&session_cookie).into()).unwrap();

    data.login_cache.lock().unwrap().insert(
        session_cookie.cookie_id.to_string(),
        Arc::new(session_cookie),
    );

    Ok(res)
}

#[post("/signup")]
async fn signup(
    login_form: Form<LoginForm>,
    data: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let user = data
        .database
        .add_user(login_form.username.clone(), login_form.password.clone())
        .await
        .map_err(AppError::SignupError)?;
    Ok(HttpResponse::Ok().body(format!("{:?}", user)))
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

use futures::StreamExt;
use reqwest::Client;

#[derive(Deserialize)]
struct ClassifyResponse {
    classification: i32,
}

#[get("/next_pending_job")]
async fn next_pending_job(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;
    let database = &state.database;
    let user = &login_cookie.user;
    //use crate::db::Job;
    let pending_jobs1 = database
        .get_user_pending_jobs(user)
        .await
        .map_err(|e| AppError::DatabaseError(e))?;

    let client = Client::new();
    for job in pending_jobs1 {
        let res = client
            .get("http://0.0.0.0:8080/classify_job")
            .query(&[
                ("job", job.to_string()),
                ("username", user.0.username),
                ("password", user.0.password_digest.clone()),
            ])
            .send()
            .await
            .map_err(|e| AppError::InternalError(e.into()))?;

        let res_json: ClassifyResponse = res
            .json()
            .await
            .map_err(|e| AppError::InternalError(e.into()))?;

        if res_json.classification == 0 {
            return Ok(web::Json(job));
        }
    }

    Err(AppError::InternalError(anyhow!("No pending jobs")))
}

#[derive(Deserialize)]
struct SearchContextReq {
    resume_text: String,
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
    let resume = database.save_resume(user, context.resume_text).await?;

    let _search_context = database
        .insert_search_context(user, resume.resume_id, context.keywords)
        .await
        .map_err(AppError::DatabaseError)?;

    Ok(HttpResponse::Ok())
}

#[get("/search_context")]
async fn get_search_context(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;

    let user = &login_cookie.user;

    let database = &state.database;

    let search_contexts = database
        .get_search_contexts_by_user(user)
        .await
        .map_err(AppError::DatabaseError)?;

    Ok(HttpResponse::Ok().body(serde_json::to_string(&search_contexts).unwrap()))
}

#[delete("/search_context/{context_id}")]
async fn delete_search_context(
    req: HttpRequest,
    context_id: Path<i32>,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;

    let user = &login_cookie.user;

    let database = &state.database;

    let _search_context = database
        .remove_search_context(user, context_id.into_inner())
        .await
        .map_err(AppError::DatabaseError)?;

    Ok(HttpResponse::Ok())
}

#[get("/active_searches")]
async fn active_searches(
    req: HttpRequest,
    state: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let login_cookie = state.verify_user(req)?;
    let user = &login_cookie.user;

    let database = &state.database;

    let search_contexts = database
        .get_search_contexts_by_user(user)
        .await
        .map_err(AppError::DatabaseError)?;

    Ok(HttpResponse::Ok().body(serde_json::to_string(&search_contexts).unwrap()))
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

pub async fn serve(database: Database) -> Result<(), anyhow::Error> {
    let app_data = AppState {
        database,
        login_cache: Mutex::new(BTreeMap::new()),
    };
    let app_data = Arc::new(app_data);

    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    HttpServer::new(move || {
        let app = App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .app_data(web::Data::new(app_data.clone()))
            .service(login)
            .service(signup)
            .service(pending_jobs)
            .service(post_search_context)
            .service(active_searches)
            .service(delete_search_context);
        app
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;
    Ok(())
}
