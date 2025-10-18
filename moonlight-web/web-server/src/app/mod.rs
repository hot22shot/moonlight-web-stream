use std::{
    ops::Deref,
    sync::{Arc, Weak},
};

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use common::config::Config;
use thiserror::Error;
use uuid::Uuid;

use crate::app::{auth::UserAuth, user::User};

pub mod auth;
pub mod host;
pub mod storage;
pub mod user;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("the app got destroyed")]
    AppDestroyed,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().finish()
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

struct AppRef {
    inner: Weak<AppInner>,
}

impl AppRef {
    fn access(&self) -> Result<impl Deref<Target = AppInner>, AppError> {
        Weak::upgrade(&self.inner).ok_or(AppError::AppDestroyed)
    }
}

struct AppInner {
    config: Config,
}

pub struct App {
    inner: Arc<AppInner>,
}

impl App {
    pub fn new(config: Config) -> Result<Self, anyhow::Error> {
        let app = AppInner { config };

        Ok(Self {
            inner: Arc::new(app),
        })
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub fn user(&self, uuid: Uuid, auth: UserAuth) -> Result<User, AppError> {
        todo!()
    }

    pub fn user_no_auth(&self, uuid: Uuid) -> Result<User, AppError> {
        todo!()
    }
}
