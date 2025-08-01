use actix_web::{
    Either, HttpResponse, Responder,
    dev::HttpServiceFactory,
    get, services,
    web::{Data, Json},
};
use moonlight_common::{
    high::{MaybePaired, MoonlightHost},
    network::ApiError,
};

use crate::{
    api_bindings::{GetHostsResponse, UndetailedHost},
    data::RuntimeApiData,
};

#[get("/authenticate")]
async fn authenticate() -> impl Responder {
    HttpResponse::Ok()
}

#[get("/hosts")]
async fn hosts(data: Data<RuntimeApiData>) -> Either<Json<GetHostsResponse>, HttpResponse> {
    let Ok(hosts) = data.hosts.read() else {
        // TODO: warn
        return Either::Right(HttpResponse::InternalServerError().finish());
    };

    let mut response_hosts = Vec::with_capacity(hosts.len());

    // TODO: parallel?
    for (host_id, host) in &*hosts {
        let Ok(mut host) = host.lock() else {
            continue;
        };

        let Ok(host) = into_undetailed_host(host_id, &mut host).await else {
            continue;
        };

        response_hosts.push(host);
    }

    Either::Left(Json(GetHostsResponse {
        hosts: response_hosts,
    }))
}
async fn into_undetailed_host(
    id: usize,
    host: &mut MoonlightHost<MaybePaired>,
) -> Result<UndetailedHost, ApiError> {
    Ok(UndetailedHost {
        host_id: id as u32,
        name: host.host_name().await?.to_string(),
        server_state: host.state().await?.1.into(),
    })
}

/// IMPORTANT: This won't authenticate clients -> everyone can use this api
/// Put a guard before this service
pub fn api_service() -> impl HttpServiceFactory {
    services![authenticate, hosts]
}
