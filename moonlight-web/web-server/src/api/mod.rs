use actix_web::{
    Error, HttpResponse, delete,
    dev::HttpServiceFactory,
    get, post, put, services,
    web::{self, Bytes, Data, Json, Query},
};
use log::{info, warn};
use moonlight_common::PairPin;
use std::io::Write as _;

use crate::{
    api::admin::add_user,
    app::{
        App, AppError,
        host::{AppId, HostId},
        user::{AuthenticatedUser, User, UserId},
    },
};
use common::api_bindings::{
    self, DeleteHostQuery, DetailedUser, GetAppImageQuery, GetAppsQuery, GetAppsResponse,
    GetHostQuery, GetHostResponse, GetHostsResponse, GetUserQuery, PairStatus, PostPairRequest,
    PostPairResponse1, PostPairResponse2, PostWakeUpRequest, PutHostRequest, PutHostResponse,
};

pub mod admin;
pub mod auth;
pub mod stream;

#[get("/user")]
async fn get_user(
    app: Data<App>,
    mut user: AuthenticatedUser,
    Query(query): Query<GetUserQuery>,
) -> Result<Json<DetailedUser>, Error> {
    async fn user_into_response(user: &mut User) -> Result<Json<DetailedUser>, Error> {
        let name = user.name().await?;
        let role = user.role().await?;

        Ok(Json(DetailedUser {
            id: user.id().0,
            name,
            role: role.into(),
        }))
    }

    match (query.name, query.user_id) {
        (None, None) => user_into_response(&mut user).await,
        (None, Some(user_id)) => {
            let user_id = UserId(user_id);

            let mut user = app.user_by_id(user_id).await?;

            user_into_response(&mut user).await
        }
        (Some(name), None) => {
            let mut user = app.user_by_name(&name).await?;

            user_into_response(&mut user).await
        }
        (Some(_), Some(_)) => Err(AppError::BadRequest.into()),
    }
}

// TODO: use response streaming to have longer timeouts on each individual host with json new line format
#[get("/hosts")]
async fn list_hosts(mut user: AuthenticatedUser) -> Result<Json<GetHostsResponse>, Error> {
    let hosts = user.hosts().await?;

    let mut undetailed_hosts = Vec::with_capacity(hosts.len());
    // TODO: do this parallel?
    for mut host in hosts {
        let undetailed = match host.undetailed_host(&mut user).await {
            Ok(value) => value,
            Err(err) => {
                // TODO: try to push the host based on cache and show error?
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
    mut user: AuthenticatedUser,
    Query(query): Query<GetHostQuery>,
) -> Result<Json<GetHostResponse>, Error> {
    let host_id = HostId(query.host_id);

    let mut host = user.host(host_id).await?;

    let detailed = host.detailed_host(&mut user).await?;

    Ok(Json(GetHostResponse { host: detailed }))
}

#[put("/host")]
async fn put_host(
    app: Data<App>,
    mut user: AuthenticatedUser,
    Json(query): Json<PutHostRequest>,
) -> Result<Json<PutHostResponse>, Error> {
    let mut host = user
        .host_add(
            query.address,
            query
                .http_port
                .unwrap_or(app.config().moonlight.default_http_port),
        )
        .await?;

    Ok(Json(PutHostResponse {
        host: host.detailed_host(&mut user).await?,
    }))
}

#[delete("/host")]
async fn delete_host(
    mut user: AuthenticatedUser,
    Query(query): Query<DeleteHostQuery>,
) -> Result<HttpResponse, AppError> {
    let host_id = HostId(query.host_id);

    user.host_delete(host_id).await?;

    Ok(HttpResponse::Ok().finish())
}

#[post("/pair")]
async fn pair_host(
    mut user: AuthenticatedUser,
    Json(request): Json<PostPairRequest>,
) -> Result<HttpResponse, AppError> {
    let host_id = HostId(request.host_id);

    let mut host = user.host(host_id).await?;

    if matches!(host.is_paired(&mut user).await?, PairStatus::Paired) {
        return Ok(HttpResponse::NotModified().finish());
    }

    let stream = async_stream::stream! {
        // Generate pin
        let Ok(pin) = PairPin::generate() else {
            warn!("[Api]: failed to generate pin!");

            return;
        };

        // Send pin response
        let Ok(text) = serde_json::to_string(&PostPairResponse1::Pin(pin.to_string())) else {
            unreachable!()
        };

        let bytes = Bytes::from_owner(text);
        yield Ok::<_, Error>(bytes);

        // Initiate pairing
        if let Err(err) = host
            .pair(
                &mut user,
                pin,
            )
            .await
        {
            info!("[Api]: failed to pair host {host:?}: {err:?}");

            let Ok(text) = serde_json::to_string(&PostPairResponse2::PairError) else {
                unreachable!()
            };

            let bytes = Bytes::from_owner(text);
            yield Ok::<_, Error>(bytes);

            return;
        };

        // Get detailed host after pairing succeeded
        let detailed_host = match host.detailed_host(&mut user).await {
            Err(err) => {
                warn!("[Api] failed to get host info after pairing for host {host:?}: {err:?}");

                let Ok(text) = serde_json::to_string(&PostPairResponse2::PairError) else {
                    unreachable!()
                };

                let bytes = Bytes::from_owner(text);
                yield Ok::<_, Error>(bytes);

                return
            }
            Ok(value) => value,
        };

        // Send detailed host back
        let mut text = Vec::new();
        let _ = writeln!(&mut text);
        if  serde_json::to_writer(&mut text, &PostPairResponse2::Paired(detailed_host)).is_err() {
            unreachable!()
        };

        let bytes = Bytes::from_owner(text);
        yield Ok::<_, Error>(bytes);
    };

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "application/x-ndjson"))
        .streaming(stream))
}

#[post("/host/wake")]
async fn wake_host(
    mut user: AuthenticatedUser,
    Json(request): Json<PostWakeUpRequest>,
) -> Result<HttpResponse, AppError> {
    let host_id = HostId(request.host_id);

    let host = user.host(host_id).await?;

    host.wake().await?;

    Ok(HttpResponse::Ok().finish())
}

#[get("/apps")]
async fn get_apps(
    mut user: AuthenticatedUser,
    Query(query): Query<GetAppsQuery>,
) -> Result<Json<GetAppsResponse>, AppError> {
    let host_id = HostId(query.host_id);

    let mut host = user.host(host_id).await?;

    let apps = host.list_apps(&mut user).await?;

    Ok(Json(GetAppsResponse {
        apps: apps
            .into_iter()
            .map(|app| api_bindings::App {
                app_id: app.id.0,
                title: app.title,
                is_hdr_supported: app.is_hdr_supported,
            })
            .collect(),
    }))
}

#[get("/app/image")]
async fn get_app_image(
    mut user: AuthenticatedUser,
    Query(query): Query<GetAppImageQuery>,
) -> Result<Bytes, AppError> {
    let host_id = HostId(query.host_id);
    let app_id = AppId(query.app_id);

    let mut host = user.host(host_id).await?;

    let image = host.app_image(&mut user, app_id).await?;

    Ok(image)
}

pub fn api_service() -> impl HttpServiceFactory {
    // TODO: take a look at: https://docs.rs/actix-web/latest/actix_web/middleware/struct.Logger.html

    web::scope("/api")
        .service(services![
            // -- Auth
            auth::login,
            auth::logout,
            auth::authenticate
        ])
        .service(services![
            // -- Host
            get_user,
            list_hosts,
            get_host,
            put_host,
            wake_host,
            delete_host,
            pair_host,
            get_apps,
            get_app_image,
        ])
        .service(services![
            // -- Stream
            stream::start_host,
            stream::cancel_host,
        ])
        .service(
            // -- Admin
            add_user,
        )
}
