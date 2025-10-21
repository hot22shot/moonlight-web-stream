use std::{
    ops::Deref,
    sync::{Arc, Weak},
};

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use common::config::Config;
use thiserror::Error;
use uuid::Uuid;

use crate::app::{
    auth::UserAuth,
    storage::{Storage, create_storage},
    user::{User, UserId},
};

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
    storage: Box<dyn Storage>,
}

pub struct App {
    inner: Arc<AppInner>,
}

impl App {
    pub async fn new(config: Config) -> Result<Self, anyhow::Error> {
        let app = AppInner {
            storage: create_storage(config.data_storage.clone()).await?,
            config,
        };

        Ok(Self {
            inner: Arc::new(app),
        })
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub async fn user(&self, id: UserId, auth: UserAuth) -> Result<User, AppError> {
        // TODO: auth
        self.user_no_auth(id).await
    }

    pub async fn user_no_auth(&self, id: UserId) -> Result<User, AppError> {
        todo!()
    }
}
