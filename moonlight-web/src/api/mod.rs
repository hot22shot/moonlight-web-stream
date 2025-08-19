use actix_web::{
    Either, Error, HttpResponse, Responder, delete,
    dev::HttpServiceFactory,
    get, middleware, post, put, services,
    web::{self, Bytes, Data, Json, Query},
};
use futures::future::join_all;
use log::{info, warn};
use moonlight_common::{
    PairStatus,
    high::HostError,
    network::{
        ApiError,
        reqwest::{ReqwestError, ReqwestMoonlightHost},
    },
    pair::generate_new_client,
};
use std::io::Write as _;
use tokio::sync::Mutex;

use crate::{
    Config,
    api::auth::{ApiCredentials, auth_middleware},
    api_bindings::{
        DeleteHostQuery, DetailedHost, GetAppImageQuery, GetAppsQuery, GetAppsResponse,
        GetHostQuery, GetHostResponse, GetHostsResponse, PostPairRequest, PostPairResponse1,
        PostPairResponse2, PutHostRequest, PutHostResponse, UndetailedHost,
    },
    data::{RuntimeApiData, RuntimeApiHost},
};

mod auth;
mod stream;

#[get("/authenticate")]
async fn authenticate() -> impl Responder {
    HttpResponse::Ok()
}

#[get("/hosts")]
async fn list_hosts(data: Data<RuntimeApiData>) -> Either<Json<GetHostsResponse>, HttpResponse> {
    let hosts = data.hosts.read().await;

    let host_results = join_all(hosts.iter().map(|(host_id, host)| async move {
        let mut host = host.lock().await;

        (
            host_id,
            into_undetailed_host(host_id, &mut host.moonlight).await,
        )
    }))
    .await;

    let host_response = host_results
        .into_iter()
        .filter_map(|(host_id, result)| match result {
            Err(err) => {
                warn!("failed to get host data {host_id}: {err:?}");

                None
            }
            Ok(value) => Some(value),
        })
        .collect::<Vec<_>>();

    Either::Left(Json(GetHostsResponse {
        hosts: host_response,
    }))
}

#[get("/host")]
async fn get_host(
    data: Data<RuntimeApiData>,
    Query(query): Query<GetHostQuery>,
) -> Either<Json<GetHostResponse>, HttpResponse> {
    let hosts = data.hosts.read().await;

    let host_id = query.host_id;
    let Some(host) = hosts.get(host_id as usize) else {
        return Either::Right(HttpResponse::NotFound().finish());
    };

    let mut host = host.lock().await;

    let Ok(detailed_host) = into_detailed_host(host_id as usize, &mut host.moonlight).await else {
        return Either::Right(HttpResponse::InternalServerError().finish());
    };

    Either::Left(Json(GetHostResponse {
        host: detailed_host,
    }))
}

#[put("/host")]
async fn put_host(
    data: Data<RuntimeApiData>,
    config: Data<Config>,
    Json(query): Json<PutHostRequest>,
) -> Either<Json<PutHostResponse>, HttpResponse> {
    // Create and Try to connect to host
    let mut host = match ReqwestMoonlightHost::new(
        query.address,
        query
            .http_port
            .unwrap_or(config.moonlight_default_http_port),
        None,
    ) {
        Ok(value) => value,
        Err(err) => {
            warn!("[Api] failed to create new moonlight host: {err:?}");
            return Either::Right(HttpResponse::InternalServerError().finish());
        }
    };

    match host.host_name().await {
        Ok(_) => {}
        Err(HostError::Api(ApiError::RequestClient(ReqwestError::Reqwest(err))))
            if err.is_timeout() =>
        {
            return Either::Right(HttpResponse::NotFound().finish());
        }
        Err(HostError::Api(ApiError::RequestClient(ReqwestError::Reqwest(err))))
            if err.is_connect() =>
        {
            return Either::Right(HttpResponse::NotFound().finish());
        }
        Err(_) => return Either::Right(HttpResponse::BadRequest().finish()),
    };

    // Write host
    let mut hosts = data.hosts.write().await;

    let host_id = hosts.vacant_key();
    hosts.insert(Mutex::new(RuntimeApiHost {
        moonlight: host,
        app_images_cache: Default::default(),
    }));

    drop(hosts);

    // Read host and respond
    let hosts = data.hosts.read().await;
    let Some(host) = hosts.get(host_id) else {
        return Either::Right(HttpResponse::InternalServerError().finish());
    };
    let mut host = host.lock().await;

    let _ = data.file_writer.try_send(());

    let Ok(detailed_host) = into_detailed_host(host_id, &mut host.moonlight).await else {
        return Either::Right(HttpResponse::InternalServerError().finish());
    };

    Either::Left(Json(PutHostResponse {
        host: detailed_host,
    }))
}

#[delete("/host")]
async fn delete_host(
    data: Data<RuntimeApiData>,
    Query(query): Query<DeleteHostQuery>,
) -> HttpResponse {
    let mut hosts = data.hosts.write().await;

    let host = hosts.try_remove(query.host_id as usize);

    drop(hosts);

    if host.is_none() {
        return HttpResponse::NotFound().finish();
    } else {
        let _ = data.file_writer.try_send(());
    }

    HttpResponse::Ok().finish()
}

#[post("/pair")]
async fn pair_host(
    data: Data<RuntimeApiData>,
    config: Data<Config>,
    Json(request): Json<PostPairRequest>,
) -> HttpResponse {
    let hosts = data.hosts.read().await;

    let host_id = request.host_id;
    let Some(host) = hosts.get(host_id as usize) else {
        return HttpResponse::NotFound().finish();
    };

    let host = host.lock().await;

    if matches!(host.moonlight.is_paired(), PairStatus::Paired) {
        return HttpResponse::NotModified().finish();
    }

    let data = data.clone();

    let stream = async_stream::stream! {
        let hosts = data.hosts.read().await;
        let Some(host) = hosts.get(host_id as usize) else {
            let Ok(text) = serde_json::to_string(&PostPairResponse1::InternalServerError) else {
                unreachable!()
            };

            let bytes = Bytes::from_owner(text);
            yield Ok::<_, Error>(bytes);

            return;
        };
        let mut host = host.lock().await;

        let Ok(client_auth) = generate_new_client() else {
            warn!("[Api]: failed to generate new client to host authentication data");

            let Ok(text) = serde_json::to_string(&PostPairResponse1::InternalServerError) else {
                unreachable!()
            };

            let bytes = Bytes::from_owner(text);
            yield Ok::<_, Error>(bytes);

            return;
        };

        let pin = data.crypto.generate_pin();

            let Ok(text) = serde_json::to_string(&PostPairResponse1::Pin(pin.to_string())) else {
                unreachable!()
            };

            let bytes = Bytes::from_owner(text);
            yield Ok::<_, Error>(bytes);

        if let Err(err) = host.moonlight
            .pair(
                &data.crypto,
                &client_auth,
                config.pair_device_name.to_string(),
                pin,
            )
            .await
        {
            info!("[Api]: failed to pair host {}: {:?}", host.moonlight.address(), err);

            let Ok(text) = serde_json::to_string(&PostPairResponse2::PairError) else {
                unreachable!()
            };

            let bytes = Bytes::from_owner(text);
            yield Ok::<_, Error>(bytes);

            return;
        };

        let detailed_host = match into_detailed_host(host_id as usize, &mut host.moonlight).await {
            Err(err) => {
                warn!("failed to get host info after pairing for host {host_id}: {err:?}");

                return
            }
            Ok(value) => value,
        };

        let mut text = Vec::new();
        let _ = writeln!(&mut text);
        if  serde_json::to_writer(&mut text, &PostPairResponse2::Paired(detailed_host)).is_err() {
            unreachable!()
        };

        drop(host);
        drop(hosts);

        let _ = data.file_writer.try_send(());

        let bytes = Bytes::from_owner(text);
        yield Ok::<_, Error>(bytes);
    };

    HttpResponse::Ok()
        .insert_header(("Content-Type", "application/x-ndjson"))
        .streaming(stream)
}

#[get("/apps")]
async fn get_apps(
    data: Data<RuntimeApiData>,
    Query(query): Query<GetAppsQuery>,
) -> Either<Json<GetAppsResponse>, HttpResponse> {
    let hosts = data.hosts.read().await;

    let host_id = query.host_id;
    let Some(host) = hosts.get(host_id as usize) else {
        return Either::Right(HttpResponse::NotFound().finish());
    };
    let mut host = host.lock().await;

    let app_list = match host.moonlight.app_list().await {
        Err(err) => {
            warn!("[Api]: failed to get app list for host {host_id}: {err:?}");

            return Either::Right(HttpResponse::InternalServerError().finish());
        }
        Ok(value) => value,
    };

    Either::Left(Json(GetAppsResponse {
        apps: app_list.into_iter().map(|x| x.to_owned().into()).collect(),
    }))
}

#[get("/app/image")]
async fn get_app_image(
    data: Data<RuntimeApiData>,
    Query(query): Query<GetAppImageQuery>,
) -> Either<Bytes, HttpResponse> {
    let hosts = data.hosts.read().await;

    let host_id = query.host_id;
    let Some(host) = hosts.get(host_id as usize) else {
        return Either::Right(HttpResponse::NotFound().finish());
    };
    let mut host = host.lock().await;

    let app_id = query.app_id;
    if let Some(cache) = host.app_images_cache.get(&app_id) {
        return Either::Left(cache.clone());
    }

    let image = host.moonlight.request_app_image(app_id).await;
    match image {
        Err(err) => {
            warn!("[Api]: failed to get host {host_id} app image {app_id}: {err:?}");

            Either::Right(HttpResponse::InternalServerError().finish())
        }
        Ok(image) => {
            host.app_images_cache.insert(app_id, image.clone());

            Either::Left(image)
        }
    }
}

pub fn api_service(data: Data<RuntimeApiData>, credentials: String) -> impl HttpServiceFactory {
    web::scope("/api")
        .wrap(middleware::from_fn(auth_middleware))
        .app_data(ApiCredentials(credentials))
        .app_data(data)
        .service(services![
            authenticate,
            list_hosts,
            get_host,
            put_host,
            delete_host,
            pair_host,
            get_apps,
            get_app_image,
            stream::start_stream,
        ])
}

async fn into_undetailed_host(
    id: usize,
    host: &mut ReqwestMoonlightHost,
) -> Result<UndetailedHost, HostError<ReqwestError>> {
    Ok(UndetailedHost {
        host_id: id as u32,
        name: host.host_name().await?.to_string(),
        paired: host.is_paired().into(),
        server_state: host.state().await?.1.into(),
    })
}
async fn into_detailed_host(
    id: usize,
    host: &mut ReqwestMoonlightHost,
) -> Result<DetailedHost, HostError<ReqwestError>> {
    Ok(DetailedHost {
        host_id: id as u32,
        name: host.host_name().await?.to_string(),
        paired: host.is_paired().into(),
        server_state: host.state().await?.1.into(),
        address: host.address().to_string(),
        http_port: host.http_port(),
        https_port: host.https_port().await?,
        external_port: host.external_port().await?,
        version: host.version().await?.to_string(),
        gfe_version: host.gfe_version().await?.to_string(),
        unique_id: host.unique_id().await?.to_string(),
        mac: host.mac().await?.to_string(),
        local_ip: host.local_ip().await?.to_string(),
        current_game: host.current_game().await?,
        max_luma_pixels_hevc: host.max_luma_pixels_hevc().await?,
        server_codec_mode_support: host.server_codec_mode_support_raw().await?,
    })
}
