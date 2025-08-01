use actix_web::{
    Either, HttpResponse, Responder,
    dev::HttpServiceFactory,
    get, services,
    web::{Data, Json, Query},
};
use moonlight_common::{
    high::{MaybePaired, MoonlightHost},
    network::ApiError,
};

use crate::{
    api_bindings::{
        DetailedHost, GetDetailedHostQuery, GetDetailedHostResponse, GetHostsResponse,
        UndetailedHost,
    },
    data::RuntimeApiData,
};

#[get("/authenticate")]
async fn authenticate() -> impl Responder {
    HttpResponse::Ok()
}

#[get("/hosts")]
async fn list_hosts(data: Data<RuntimeApiData>) -> Either<Json<GetHostsResponse>, HttpResponse> {
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

#[get("/host")]
async fn get_detailed_host(
    data: Data<RuntimeApiData>,
    query: Query<GetDetailedHostQuery>,
) -> Either<Json<GetDetailedHostResponse>, HttpResponse> {
    let Ok(hosts) = data.hosts.read() else {
        // TODO: warn
        return Either::Right(HttpResponse::InternalServerError().finish());
    };

    let host_id = query.host_id;
    let Some(host) = hosts.get(host_id as usize) else {
        return Either::Right(HttpResponse::NotFound().finish());
    };

    let Ok(mut host) = host.lock() else {
        return Either::Right(HttpResponse::InternalServerError().finish());
    };

    let Ok(detailed_host) = into_detailed_host(host_id as usize, &mut host).await else {
        return Either::Right(HttpResponse::InternalServerError().finish());
    };

    Either::Left(Json(GetDetailedHostResponse {
        host: detailed_host,
    }))
}

/// IMPORTANT: This won't authenticate clients -> everyone can use this api
/// Put a guard before this service
pub fn api_service() -> impl HttpServiceFactory {
    services![authenticate, list_hosts, get_detailed_host]
}

async fn into_undetailed_host(
    id: usize,
    host: &mut MoonlightHost<MaybePaired>,
) -> Result<UndetailedHost, ApiError> {
    Ok(UndetailedHost {
        host_id: id as u32,
        name: host.host_name().await?.to_string(),
        paired: host.is_paired().into(),
        server_state: host.state().await?.1.into(),
    })
}
async fn into_detailed_host(
    id: usize,
    host: &mut MoonlightHost<MaybePaired>,
) -> Result<DetailedHost, ApiError> {
    Ok(DetailedHost {
        host_id: id as u32,
        name: host.host_name().await?.to_string(),
        paired: host.is_paired().into(),
        server_state: host.state().await?.1.into(),
        https_port: host.https_port().await?,
        external_port: host.external_port().await?,
        version: host.version().await?.to_string(),
        gfe_version: host.gfe_version().await?.to_string(),
        unique_id: host.unique_id().await?.to_string(),
        mac: host.mac().await?.to_string(),
        local_ip: host.local_ip().await?.to_string(),
        current_game: host.current_game().await?,
        max_luma_pixels_hevc: host.max_luma_pixels_hevc().await?,
        server_codec_mode_support: host.server_codec_mode_support().await?.bits(),
    })
}
