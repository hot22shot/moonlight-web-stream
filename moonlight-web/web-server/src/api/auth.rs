use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use futures::future::{Ready, err, ok};

use crate::app::{App, AppError, user::User};

impl FromRequest for User {
    type Error = AppError;

    type Future = Ready<Result<Self, Self::Error>>;

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

        let user = match app.user(uuid, auth) {
            Err(error) => {
                return err(error);
            }
            Ok(value) => value,
        };

        ok(user)
    }
}
