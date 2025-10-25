use actix_web::{
    HttpResponse, put,
    web::{Data, Json},
};
use common::api_bindings::PutUserRequest;

use crate::app::{App, AppError, password::StoragePassword, storage::StorageUserAdd};

#[put("/user")]
pub async fn add_user(
    app: Data<App>,
    // TODO: secure this
    // admin: Admin,
    Json(request): Json<PutUserRequest>,
) -> Result<HttpResponse, AppError> {
    let _user = app
        .add_user_no_auth(StorageUserAdd {
            name: request.name,
            password: StoragePassword::new(&request.password)?,
            role: request.role.into(),
        })
        .await?;

    Ok(HttpResponse::Ok().finish())
}
