use actix_web::{
    guard::{Guard, GuardContext},
    http::header,
    web::Data,
};

use crate::Config;

pub struct AuthGuard;

impl Guard for AuthGuard {
    fn check(&self, ctx: &GuardContext<'_>) -> bool {
        let Some(config) = ctx.app_data::<Data<Config>>() else {
            return false;
        };

        let Some(value) = ctx
            .head()
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
        else {
            return false;
        };

        let Some((auth_type, credentials)) = value.split_once(" ") else {
            return false;
        };

        auth_type == "Bearer" && credentials == config.credentials
    }
}
