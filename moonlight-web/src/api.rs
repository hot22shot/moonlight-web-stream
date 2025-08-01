use actix_web::{HttpResponse, Responder, dev::HttpServiceFactory, get, post, services, web::Json};

use crate::api_bindings::{TestRequest, TestResponse};

#[get("/authenticate")]
async fn authenticate() -> impl Responder {
    HttpResponse::Ok()
}

#[post("/echo")]
async fn echo(Json(request): Json<TestRequest>) -> Json<TestResponse> {
    Json(TestResponse {
        world: request.hello,
    })
}

/// IMPORTANT: This won't authenticate clients -> everyone can use this api
/// But a guard before this service
pub fn api_service() -> impl HttpServiceFactory {
    services![authenticate, echo]
}
