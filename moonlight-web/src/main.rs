use actix_web::{App, HttpServer, post, web::Json};

use crate::api_bindings::{TestRequest, TestResponse};

mod api_bindings;
#[cfg(feature = "include-web")]
mod web;

#[post("api/echo")]
async fn echo(Json(request): Json<TestRequest>) -> Json<TestResponse> {
    Json(TestResponse {
        world: request.hello,
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let address = "127.0.0.1";
    let port = 8080;

    println!("Starting server on http://{address}:{port}");

    HttpServer::new(|| {
        let app = App::new().service(echo);

        #[cfg(feature = "include-web")]
        let app = app.service(web::web_service());

        app
    })
    .bind((address, port))?
    .run()
    .await
}
