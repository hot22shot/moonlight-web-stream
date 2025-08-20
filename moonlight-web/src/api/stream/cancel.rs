use actix_web::{
    Either, HttpResponse, post,
    web::{Data, Json},
};
use log::warn;

use crate::{
    api_bindings::{PostCancelRequest, PostCancelResponse},
    data::RuntimeApiData,
};

#[post("/host/cancel")]
pub async fn host_cancel(
    data: Data<RuntimeApiData>,
    request: Json<PostCancelRequest>,
) -> Either<Json<PostCancelResponse>, HttpResponse> {
    let hosts = data.hosts.read().await;

    let host_id = request.host_id;
    let Some(host) = hosts.get(host_id as usize) else {
        return Either::Right(HttpResponse::NotFound().finish());
    };

    let mut host = host.lock().await;

    let success = match host.moonlight.cancel().await {
        Ok(value) => value,
        Err(err) => {
            warn!("[Api]: failed to cancel stream for {host_id}:{err:?}");

            return Either::Right(HttpResponse::InternalServerError().finish());
        }
    };

    Either::Left(Json(PostCancelResponse { success }))
}
