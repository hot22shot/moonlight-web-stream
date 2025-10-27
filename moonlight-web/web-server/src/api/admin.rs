use actix_web::{
    HttpResponse, get, put,
    web::{Data, Json},
};
use common::api_bindings::{self, GetUsersResponse, PutUserRequest};
use futures::future::join_all;
use log::warn;

use crate::app::{App, AppError, password::StoragePassword, storage::StorageUserAdd, user::Admin};

#[put("/user")]
pub async fn add_user(
    app: Data<App>,
    admin: Admin,
    Json(request): Json<PutUserRequest>,
) -> Result<HttpResponse, AppError> {
    let _user = app
        .add_user(
            &admin,
            StorageUserAdd {
                name: request.name,
                password: StoragePassword::new(&request.password)?,
                role: request.role.into(),
            },
        )
        .await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/users")]
pub async fn list_users(app: Data<App>, admin: Admin) -> Result<Json<GetUsersResponse>, AppError> {
    let mut users = app.all_users(admin).await?;

    let user_results = join_all(users.iter_mut().map(|user| user.detailed_user_no_auth())).await;

    let mut out_users = Vec::with_capacity(user_results.len());
    for (result, user) in user_results.into_iter().zip(users) {
        match result {
            Ok(user) => {
                out_users.push(user);
            }
            Err(err) => {
                warn!("Failed to query detailed user of {user:?}: {err:?}");
            }
        }
    }

    Ok(Json(GetUsersResponse { users: out_users }))
}
