use std::pin::Pin;

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};

use crate::app::{
    App, AppError,
    auth::{SessionToken, UserAuth},
    user::{Admin, Role, User},
};

pub const COOKIE_SESSION_TOKEN_NAME: &str = "mlSession";

impl FromRequest for User {
    type Error = AppError;

    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let app = match req.app_data::<Data<App>>() {
            None => {
                // TODO
                todo!()
            }
            Some(value) => value,
        };

        // TODO: look for forwarded headers

        let auth = if let Some(bearer) = req.headers().get("Authorization") {
            // Look for bearer
            // TODO: error handling, use header Malformed request?
            let bearer = bearer.to_str().unwrap();

            let token_str = bearer.strip_prefix("Bearer").unwrap();

            let token = SessionToken::decode(token_str).unwrap();

            UserAuth::Session(token)
        } else if let Some(cookie) = req.cookie(COOKIE_SESSION_TOKEN_NAME) {
            // Look for cookie
            // TODO: error handling
            let token = SessionToken::decode(cookie.value()).unwrap();

            UserAuth::Session(token)
        } else {
            UserAuth::None
        };

        let app = app.clone();
        Box::pin(async move {
            let user = app.user(auth).await?;

            Ok(user)
        })
    }
}

impl FromRequest for Admin {
    type Error = AppError;

    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let future = User::from_request(req, payload);

        Box::pin(async move {
            let user = future.await?;

            Admin::try_from(user).await
        })
    }
}
