// logout
// // request_cover_letter
// input_cover_letter
// input_relevant_info
// pending_job_actions = (reject, proposal)
// request_proposal(job_id)

use std::{
    collections::BTreeMap,
    future::pending,
    sync::{Arc, Mutex},
    time::Instant,
};

use actix_web::{
    cookie::{time::Duration, Cookie},
    get, post,
    web::{self, Data, Form, Json},
    App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError,
};
use anyhow::anyhow;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    db::{Database, Job, Resume, SearchContext, User, VerifiedUser},
    db_utils::{FetchId, Index},
};

#[derive(PartialEq, PartialOrd, Eq, Ord)]
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
            .get(session_id.into())
            .map(|inner| inner.clone())
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
}

impl ResponseError for AppError {}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}
#[post("/login")]
async fn login(
    req: HttpRequest,
    login_form: Form<LoginForm>,
    data: Data<Arc<AppState>>,
) -> Result<impl Responder, AppError> {
    let user = data
        .database
        .get_user(login_form.username.clone(), login_form.password.clone())
        .await
        .map_err(|err| AppError::LoginError(err))?;
    let session_cookie = LoginCookie::new(user, Duration::hours(1));
    let mut res = HttpResponse::Ok().body(format!("login successful"));

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
        .map_err(|err| AppError::SignupError(err))?;
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
        .get_user_pending_jobs(user.0.username.clone())
        .await
        .map_err(|err| AppError::DatabaseError(err))?;

    Ok(HttpResponse::Ok().body(serde_json::to_string(&pending_jobs).unwrap()))
}

#[derive(Deserialize)]
struct SearchContextReq {
    resume: String,
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
    let user_id = &user.0.user_id;

    let context = context.into_inner();

    let pool = &state.database.pool;
    let _search_context = user
        .insert_search_context(context.resume_id, context.keywords, pool)
        .await
        .map_err(|err| AppError::DatabaseError(err))?;

    Ok(HttpResponse::Ok())
}

pub async fn serve() -> Result<(), anyhow::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://shmendez@localhost/gptftw")
        .await?;
    let app_data = AppState {
        database: Database::new(pool),
        login_cache: Mutex::new(BTreeMap::new()),
    };
    let app_data = Arc::new(app_data);
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_data.clone()))
            .service(login)
            .service(signup)
            .service(pending_jobs)
            .service(post_search_context)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;
    Ok(())
}
