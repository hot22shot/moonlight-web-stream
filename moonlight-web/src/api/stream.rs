use actix_web::{
    Either, HttpResponse, post,
    web::{Data, Json},
};
use webrtc::{
    api::{
        APIBuilder, interceptor_registry::register_default_interceptors, media_engine::MediaEngine,
    },
    ice_transport::{ice_candidate::RTCIceCandidate, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::configuration::RTCConfiguration,
};

use crate::{
    api_bindings::{PostStartStreamRequest1, PostStartStreamResponse1},
    data::RuntimeApiData,
};

#[post("/start_stream")]
pub async fn start_stream(
    data: Data<RuntimeApiData>,
    Json(request): Json<PostStartStreamRequest1>,
) -> Either<Json<PostStartStreamResponse1>, HttpResponse> {
    todo!()
}

async fn start_rtc(candidates: Vec<RTCIceCandidate>) -> Result<(), anyhow::Error> {
    let config = RTCConfiguration {
        // TODO: put this into config
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let mut media = MediaEngine::default();
    // TODO: only register supported codecs
    media.register_default_codecs()?;

    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut media)?;

    let api = APIBuilder::new()
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build();

    let peer_connection = api.new_peer_connection(config).await?;

    let offer_address = "";

    Ok(())
}
