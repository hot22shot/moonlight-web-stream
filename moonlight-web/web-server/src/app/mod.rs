use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Weak},
};

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use common::config::Config;
use hex::FromHexError;
use moonlight_common::{
    high::{HostError, PairInfo},
    network::{
        ApiError, ClientInfo,
        request_client::RequestClient,
        reqwest::{ReqwestClient, ReqwestError, ReqwestMoonlightHost},
    },
};
use openssl::error::ErrorStack;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::app::{
    auth::{SessionToken, UserAuth},
    host::HostId,
    storage::{Storage, StorageUserAdd, create_storage},
    user::{Admin, User, UserId},
};

pub mod auth;
pub mod host;
pub mod password;
pub mod storage;
pub mod user;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("the app got destroyed")]
    AppDestroyed,
    #[error("the user was not found")]
    UserNotFound,
    #[error("the host was not found")]
    HostNotFound,
    #[error("the host was is already paired")]
    HostPaired,
    #[error("the host must be paired for this action")]
    HostNotPaired,
    #[error("the host was offline, but the action requires that the host is online")]
    HostOffline,
    // TODO: rename the credentials error
    #[error("the credentials don't exists")]
    CredentialsWrong,
    #[error("the host was not found")]
    SessionTokenNotFound,
    // CredentialsWrong and SessionToken not found describe this more exact
    #[error("the action is not allowed because the user is not authorized, 401")]
    Unauthorized,
    #[error("the action is not allowed with the current privileges, 403")]
    Forbidden,
    #[error("the authorization header is not a bearer")]
    AuthorizationNotBearer,
    #[error("openssl error occured")]
    OpenSSL(#[from] ErrorStack),
    #[error("hex error occured")]
    Hex(#[from] FromHexError),
    #[error("moonlight api error")]
    MoonlightApi(#[from] ApiError<<MoonlightClient as RequestClient>::Error>),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::new(self.status_code())
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::AppDestroyed => StatusCode::INTERNAL_SERVER_ERROR,
            Self::HostNotFound => StatusCode::NOT_FOUND,
            Self::HostNotPaired => StatusCode::FORBIDDEN,
            Self::HostPaired => StatusCode::NOT_MODIFIED,
            Self::HostOffline => StatusCode::GATEWAY_TIMEOUT,
            Self::UserNotFound => StatusCode::NOT_FOUND,
            Self::CredentialsWrong => StatusCode::UNAUTHORIZED,
            Self::SessionTokenNotFound => StatusCode::UNAUTHORIZED,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::OpenSSL(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Hex(_) => StatusCode::BAD_REQUEST,
            Self::AuthorizationNotBearer => StatusCode::BAD_REQUEST,
            Self::MoonlightApi(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Clone)]
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
    storage: Arc<dyn Storage + Send + Sync>,
}

pub type MoonlightClient = ReqwestClient;

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

    fn new_ref(&self) -> AppRef {
        AppRef {
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    /// admin: The admin that tries to do this action
    pub async fn add_user(&self, _: &Admin, user: StorageUserAdd) -> Result<User, AppError> {
        self.add_user_no_auth(user).await
    }

    pub async fn add_user_no_auth(&self, user: StorageUserAdd) -> Result<User, AppError> {
        // TODO: use storage user
        let user = self.inner.storage.add_user(user).await?;

        Ok(User {
            app: self.new_ref(),
            id: user.id,
        })
    }

    pub async fn user(&self, auth: UserAuth) -> Result<User, AppError> {
        match auth {
            UserAuth::None => {
                // TODO: allow a default user to exist
                Err(AppError::Unauthorized)
            }
            UserAuth::UserPassword { username, password } => {
                self.user_by_name_password(&username, &password).await
            }
            UserAuth::Session(session) => {
                let user = self.user_by_session(session).await?;

                Ok(user)
            }
            UserAuth::ForwardedHeaders { username } => {
                // TODO: look if config enabled so we can trust or not
                todo!()
            }
        }
    }
    pub async fn user_by_name_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<User, AppError> {
        let user = self.user_by_name_no_auth(username).await?;

        if !user.verify_password(password).await? {
            return Err(AppError::CredentialsWrong);
        }

        Ok(user)
    }

    pub async fn user_no_auth(&self, id: UserId) -> Result<User, AppError> {
        let user = self.inner.storage.get_user(id).await?;

        Ok(User {
            app: self.new_ref(),
            id: user.id,
        })
    }
    pub async fn user_by_name_no_auth(&self, name: &str) -> Result<User, AppError> {
        let (user_id, user) = self.inner.storage.get_user_by_name(name).await?;

        // TODO: use optional user field

        Ok(User {
            app: self.new_ref(),
            id: user_id,
        })
    }
    pub async fn user_by_session(&self, session: SessionToken) -> Result<User, AppError> {
        let (user_id, user) = self
            .inner
            .storage
            .get_user_by_session_token(session)
            .await?;

        // TODO: use optional user field

        Ok(User {
            app: self.new_ref(),
            id: user_id,
        })
    }

    pub async fn delete_session(&self, session: SessionToken) -> Result<(), AppError> {
        self.inner.storage.remove_session_token(session).await
    }
}
