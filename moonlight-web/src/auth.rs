use actix_web::{
    Error, HttpResponse,
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::header,
    middleware::Next,
    web::Data,
};

use crate::Config;

pub async fn auth_middleware(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    if authenticate(&req) {
        next.call(req).await
    } else {
        let response = HttpResponse::Unauthorized().finish();

        Ok(req.into_response(response))
    }
}

fn authenticate(request: &ServiceRequest) -> bool {
    let Some(config) = request.app_data::<Data<Config>>() else {
        return false;
    };

    let Some(value) = request
        .head()
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    let Some((auth_type, credentials)) = value.split_once(" ") else {
        todo!()
    };

    auth_type == "Bearer" && credentials == config.credentials
}
