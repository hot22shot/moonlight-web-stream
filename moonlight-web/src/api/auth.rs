use actix_web::{
    Error, HttpResponse,
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::header,
    middleware::Next,
};

pub struct ApiCredentials(pub String);

pub async fn auth_middleware(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    if req.uri().path() == "/api/stream" {
        // This will route the stream web socket through
        // because web socket cannot have the auth header
        // The Ws is authenticated in the start_stream handler
        return next.call(req).await;
    }

    if authenticate(&req) {
        next.call(req).await
    } else {
        let response = HttpResponse::Unauthorized().finish();

        Ok(req.into_response(response))
    }
}

fn authenticate(request: &ServiceRequest) -> bool {
    let Some(credentials) = request.app_data::<ApiCredentials>() else {
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

    let Some((auth_type, request_credentials)) = value.split_once(" ") else {
        todo!()
    };

    auth_type == "Bearer" && request_credentials == credentials.0.as_str()
}
