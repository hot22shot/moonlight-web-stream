use actix_web::{HttpResponse, Responder, dev::HttpServiceFactory, get, services};

pub fn web_service() -> impl HttpServiceFactory {
    services![index]
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(include_str!("../dist/index.html"))
}
