use actix_web::{
    Either, Error, HttpResponse, Responder, delete,
    dev::HttpServiceFactory,
    get, middleware, post, put, services,
    web::{self, Bytes, Data, Json, Query},
};
use futures::future::join_all;
use log::{info, warn};
use moonlight_common::{
    PairPin, PairStatus,
    high::{HostError, broadcast_magic_packet},
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
    app::{
        App,
        host::{Host, HostId},
        user::User,
    },
};
use common::api_bindings::{
    DeleteHostQuery, DetailedHost, GetAppImageQuery, GetAppsQuery, GetAppsResponse, GetHostQuery,
    GetHostResponse, GetHostsResponse, PostPairRequest, PostPairResponse1, PostPairResponse2,
    PostWakeUpRequest, PutHostRequest, PutHostResponse, UndetailedHost,
};

pub mod auth;
// mod stream;

#[get("/authenticate")]
async fn authenticate(_user: User) -> impl Responder {
    HttpResponse::Ok()
}

#[get("/hosts")]
async fn list_hosts(user: User) -> Result<Json<GetHostsResponse>, Error> {
    let hosts = user.hosts().await?;

    let mut undetailed_hosts = Vec::with_capacity(hosts.len());
    for host in hosts {
        let undetailed = match host.undetailed_host().await {
            Ok(value) => value,
            Err(err) => {
                warn!("[Api] Failed to get undetailed data of host {host:?}: {err:?}");
                continue;
            }
        };

        undetailed_hosts.push(undetailed);
    }

    Ok(Json(GetHostsResponse {
        hosts: undetailed_hosts,
    }))
}

#[get("/host")]
async fn get_host(
    user: User,
    Query(query): Query<GetHostQuery>,
) -> Result<Json<GetHostResponse>, Error> {
    let host_id = HostId(query.host_id);

    let host = user.host(host_id).await?;

    let detailed = host.detailed_host().await?;

    Ok(Json(GetHostResponse { host: detailed }))
}

#[put("/host")]
async fn put_host(
    app: Data<App>,
    user: User,
    Json(query): Json<PutHostRequest>,
) -> Result<Json<PutHostResponse>, Error> {
    let host = user
        .host_add(
            query.address,
            query
                .http_port
                .unwrap_or(app.config().moonlight.default_http_port),
        )
        .await?;

    Ok(Json(PutHostResponse {
        host: host.detailed_host().await?,
    }))
}
//
// #[delete("/host")]
// async fn delete_host(
//     data: Data<RuntimeApiData>,
//     Query(query): Query<DeleteHostQuery>,
// ) -> HttpResponse {
//     let mut hosts = data.hosts.write().await;
//
//     let host = hosts.try_remove(query.host_id as usize);
//
//     drop(hosts);
//
//     if host.is_none() {
//         return HttpResponse::NotFound().finish();
//     } else {
//         let _ = data.file_writer.try_send(());
//     }
//
//     HttpResponse::Ok().finish()
// }
//
// #[post("/pair")]
// async fn pair_host(
//     data: Data<RuntimeApiData>,
//     config: Data<Config>,
//     Json(request): Json<PostPairRequest>,
// ) -> HttpResponse {
//     let hosts = data.hosts.read().await;
//
//     let host_id = request.host_id;
//     let Some(host) = hosts.get(host_id as usize) else {
//         return HttpResponse::NotFound().finish();
//     };
//
//     let host = host.lock().await;
//
//     if matches!(host.moonlight.pair_status(), PairStatus::Paired) {
//         return HttpResponse::NotModified().finish();
//     }
//
//     let data = data.clone();
//
//     let stream = async_stream::stream! {
//         let hosts = data.hosts.read().await;
//         let Some(host) = hosts.get(host_id as usize) else {
//             let Ok(text) = serde_json::to_string(&PostPairResponse1::InternalServerError) else {
//                 unreachable!()
//             };
//
//             let bytes = Bytes::from_owner(text);
//             yield Ok::<_, Error>(bytes);
//
//             return;
//         };
//         let mut host = host.lock().await;
//
//         let Ok(client_auth) = generate_new_client() else {
//             warn!("[Api]: failed to generate new client to host authentication data");
//
//             let Ok(text) = serde_json::to_string(&PostPairResponse1::InternalServerError) else {
//                 unreachable!()
//             };
//
//             let bytes = Bytes::from_owner(text);
//             yield Ok::<_, Error>(bytes);
//
//             return;
//         };
//
//         let Ok(pin) = PairPin::generate() else {
//             warn!("[Api]: failed to generate pin!");
//
//             return
//         };
//
//             let Ok(text) = serde_json::to_string(&PostPairResponse1::Pin(pin.to_string())) else {
//                 unreachable!()
//             };
//
//             let bytes = Bytes::from_owner(text);
//             yield Ok::<_, Error>(bytes);
//
//         if let Err(err) = host.moonlight
//             .pair(
//                 &client_auth,
//                 config.pair_device_name.to_string(),
//                 pin,
//             )
//             .await
//         {
//             info!("[Api]: failed to pair host {}: {:?}", host.moonlight.address(), err);
//
//             let Ok(text) = serde_json::to_string(&PostPairResponse2::PairError) else {
//                 unreachable!()
//             };
//
//             let bytes = Bytes::from_owner(text);
//             yield Ok::<_, Error>(bytes);
//
//             return;
//         };
//
//         let _ = data.file_writer.try_send(());
//
//         let detailed_host = match into_detailed_host(host_id as usize, &mut host.moonlight).await {
//             Err(err) => {
//                 warn!("[Api] failed to get host info after pairing for host {host_id}: {err:?}");
//
//                 let Ok(text) = serde_json::to_string(&PostPairResponse2::PairError) else {
//                     unreachable!()
//                 };
//
//                 let bytes = Bytes::from_owner(text);
//                 yield Ok::<_, Error>(bytes);
//
//                 return
//             }
//             Ok(value) => value,
//         };
//
//         let mut text = Vec::new();
//         let _ = writeln!(&mut text);
//         if  serde_json::to_writer(&mut text, &PostPairResponse2::Paired(detailed_host)).is_err() {
//             unreachable!()
//         };
//
//         drop(host);
//         drop(hosts);
//
//         let bytes = Bytes::from_owner(text);
//         yield Ok::<_, Error>(bytes);
//     };
//
//     HttpResponse::Ok()
//         .insert_header(("Content-Type", "application/x-ndjson"))
//         .streaming(stream)
// }
//
// #[post("/host/wake")]
// async fn wake_host(
//     data: Data<RuntimeApiData>,
//     Json(request): Json<PostWakeUpRequest>,
// ) -> HttpResponse {
//     let hosts = data.hosts.read().await;
//
//     let host_id = request.host_id;
//     let Some(host) = hosts.get(host_id as usize) else {
//         return HttpResponse::NotFound().finish();
//     };
//     let host = host.lock().await;
//
//     let mac = host.cache.mac;
//
//     if let Some(mac) = mac {
//         if let Err(err) = broadcast_magic_packet(mac).await {
//             warn!("failed to send magic(wake on lan) packet: {err:?}");
//             return HttpResponse::InternalServerError().finish();
//         }
//     } else {
//         return HttpResponse::InternalServerError().finish();
//     }
//
//     HttpResponse::Ok().finish()
// }
//
// #[get("/apps")]
// async fn get_apps(
//     data: Data<RuntimeApiData>,
//     Query(query): Query<GetAppsQuery>,
// ) -> Either<Json<GetAppsResponse>, HttpResponse> {
//     let hosts = data.hosts.read().await;
//
//     let host_id = query.host_id;
//     let Some(host) = hosts.get(host_id as usize) else {
//         return Either::Right(HttpResponse::NotFound().finish());
//     };
//     let mut host = host.lock().await;
//
//     if query.force_refresh {
//         host.moonlight.clear_cache();
//     }
//
//     let app_list = match host.moonlight.app_list().await {
//         Err(err) => {
//             warn!("[Api]: failed to get app list for host {host_id}: {err:?}");
//
//             return Either::Right(HttpResponse::InternalServerError().finish());
//         }
//         Ok(value) => value,
//     };
//
//     Either::Left(Json(GetAppsResponse {
//         apps: app_list.iter().map(|x| x.to_owned().into()).collect(),
//     }))
// }
//
// #[get("/app/image")]
// async fn get_app_image(
//     data: Data<RuntimeApiData>,
//     Query(query): Query<GetAppImageQuery>,
// ) -> Either<Bytes, HttpResponse> {
//     let hosts = data.hosts.read().await;
//
//     let host_id = query.host_id;
//     let Some(host) = hosts.get(host_id as usize) else {
//         return Either::Right(HttpResponse::NotFound().finish());
//     };
//     let mut host = host.lock().await;
//
//     if query.force_refresh {
//         host.app_images_cache.clear();
//         host.moonlight.clear_cache();
//     }
//
//     let app_id = query.app_id;
//     if let Some(cache) = host.app_images_cache.get(&app_id) {
//         return Either::Left(cache.clone());
//     }
//
//     let image = host.moonlight.request_app_image(app_id).await;
//     match image {
//         Err(err) => {
//             warn!("[Api]: failed to get host {host_id} app image {app_id}: {err:?}");
//
//             Either::Right(HttpResponse::InternalServerError().finish())
//         }
//         Ok(image) => {
//             host.app_images_cache.insert(app_id, image.clone());
//
//             Either::Left(image)
//         }
//     }
// }

pub fn api_service() -> impl HttpServiceFactory {
    web::scope("/api")
        // .wrap(middleware::from_fn(auth_middleware))
        .service(services![
            authenticate,
            // stream::start_host,
            // stream::cancel_host,
            list_hosts,
            get_host,
            put_host,
            // wake_host,
            // delete_host,
            // pair_host,
            // get_apps,
            // get_app_image,
        ])
}

// async fn into_undetailed_host(
//     id: usize,
//     name: impl FnOnce() -> String,
//     host: &mut ReqwestMoonlightHost,
// ) -> UndetailedHost {
//     let name = host
//         .host_name()
//         .await
//         .map(str::to_string)
//         .unwrap_or_else(|_| name());
//
//     let paired = host.is_paired();
//
//     let server_state = host
//         .state()
//         .await
//         .map(|(_, state)| Option::Some(state))
//         .unwrap_or(None);
//
//     UndetailedHost {
//         host_id: id as u32,
//         name,
//         paired: paired.into(),
//         server_state: server_state.map(Into::into),
//     }
// }
// async fn into_detailed_host(
//     id: usize,
//     host: &mut ReqwestMoonlightHost,
// ) -> Result<DetailedHost, HostError<ReqwestError>> {
//     Ok(DetailedHost {
//         host_id: id as u32,
//         name: host.host_name().await?.to_string(),
//         paired: host.is_paired().into(),
//         server_state: host.state().await?.1.into(),
//         address: host.address().to_string(),
//         http_port: host.http_port(),
//         https_port: host.https_port().await?,
//         external_port: host.external_port().await?,
//         version: host.version().await?.to_string(),
//         gfe_version: host.gfe_version().await?.to_string(),
//         unique_id: host.unique_id().await?.to_string(),
//         mac: host.mac().await?.map(|mac| mac.to_string()),
//         local_ip: host.local_ip().await?.to_string(),
//         current_game: host.current_game().await?,
//         max_luma_pixels_hevc: host.max_luma_pixels_hevc().await?,
//         server_codec_mode_support: host.server_codec_mode_support_raw().await?,
//     })
// }
//
