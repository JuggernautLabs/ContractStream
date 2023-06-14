use std::{sync::Arc, time::Instant};

use actix_web::{cookie::time::Duration, HttpRequest, ResponseError};
// Add this line
//use tokio_stream::stream_ext::StreamExt;
use dashmap::DashMap;

use reqwest::StatusCode;
use thiserror::Error;
use uuid::Uuid;

use crate::db::{Database, VerifiedUser};

type SessionId = String;
type Username = String;
#[derive(PartialEq)]
pub struct LoginCookie {
    pub cookie_id: Uuid,
    death_date: Instant,
    pub user: VerifiedUser,
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

#[derive(Error, Debug)]
pub enum AppError {
    #[error("login failed")]
    LoginError(anyhow::Error),
    #[error("signup failed")]
    SignupError(anyhow::Error),
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
            AppError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InvalidSession => StatusCode::UNAUTHORIZED,
            AppError::InvalidShape(_) => StatusCode::BAD_REQUEST,
            AppError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// users only ever have a single session
/// If a user is logging in we check `username_session` to see if the user has already in `login_cache`
/// if not, the user is added to username_session and login_cache
/// a new cookie is only created if the user is not already cached
/// this prevents the size of AppState from blowing up by single users logging in multiple times
pub struct AppState {
    pub database: Database,
    username_session: DashMap<Username, SessionId>,
    login_cache: DashMap<SessionId, Arc<LoginCookie>>,
}

impl AppState {
    pub async fn is_logged_in(&self, user: &VerifiedUser) -> Option<SessionId> {
        let res = self
            .username_session
            .get(&user.0.username)
            .as_deref()
            .cloned();
        res
    }
    pub async fn verify_user(&self, req: HttpRequest) -> Result<Arc<LoginCookie>, AppError> {
        let cookie = req.cookie("session_id").ok_or(AppError::InvalidSession)?;
        let session_id = cookie.value();
        let res: Result<Arc<LoginCookie>, AppError> = self
            .login_cache
            .get(session_id)
            .as_deref()
            .cloned()
            .ok_or(AppError::InvalidSession);

        res
    }
    pub async fn login(&self, user: VerifiedUser) -> Result<Arc<LoginCookie>, AppError> {
        if let Some(session_id) = self.is_logged_in(&user).await {
            let res: Result<Arc<LoginCookie>, AppError> = self
                .login_cache
                .get(&session_id)
                .as_deref()
                .cloned()
                .ok_or(AppError::InvalidSession);
            res
        } else {
            let username = user.0.username.clone();
            let session_cookie = Arc::new(LoginCookie::new(user, Duration::hours(1)));

            let session_id: SessionId = session_cookie.cookie_id.to_string();

            self.username_session.insert(username, session_id.clone());
            self.login_cache.insert(session_id, session_cookie.clone());
            Ok(session_cookie)
        }
    }

    pub(crate) fn new(database: Database) -> Self {
        AppState {
            database,
            login_cache: DashMap::new(),
            username_session: DashMap::new(),
        }
    }
}
