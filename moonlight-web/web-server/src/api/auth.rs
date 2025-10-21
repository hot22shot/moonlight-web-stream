use std::pin::Pin;

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};

use crate::app::{App, AppError, user::User};

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

        let uuid = todo!();
        let auth = todo!();

        let app = app.clone();
        Box::pin(async move {
            let user = app.user(uuid, auth).await?;

            Ok(user)
        })
    }
}
