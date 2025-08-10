use std::io::Write;

use roxmltree::Document;
use uuid::fmt::Hyphenated;

use crate::{
    moonlight::MoonlightInstance,
    network::{
        ApiError, ClientInfo,
        request_client::{
            DynamicQueryParams, QueryBuilder, RequestClient, query_param, query_param_owned,
        },
        xml_child_text, xml_root_node,
    },
};

#[derive(Debug, Clone)]
pub struct ClientStreamRequest {
    pub app_id: u32,
    pub mode_width: u32,
    pub mode_height: u32,
    pub mode_fps: u32,
    pub ri_key: [u8; 16usize],
    pub ri_key_id: i32,
}

#[derive(Debug, Clone)]
pub struct HostLaunchResponse {
    pub game_session: u32,
    pub rtsp_session_url: String,
}

pub async fn host_launch<C: RequestClient>(
    instance: &MoonlightInstance,
    client: &mut C,
    https_address: &str,
    info: ClientInfo<'_>,
    request: ClientStreamRequest,
) -> Result<HostLaunchResponse, ApiError<C::Error>> {
    let response =
        inner_launch_host(instance, client, https_address, "launch", info, request).await?;

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    Ok(HostLaunchResponse {
        game_session: xml_child_text::<C>(root, "gamesession")?.parse()?,
        rtsp_session_url: xml_child_text::<C>(root, "sessionUrl0")?.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct HostResumeResponse {
    pub resume: u32,
    pub rtsp_session_url: String,
}

pub async fn host_resume<C: RequestClient>(
    instance: &MoonlightInstance,
    client: &mut C,
    https_hostport: &str,
    info: ClientInfo<'_>,
    request: ClientStreamRequest,
) -> Result<HostResumeResponse, ApiError<C::Error>> {
    let response =
        inner_launch_host(instance, client, https_hostport, "resume", info, request).await?;

    let doc = Document::parse(response.as_ref())?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    Ok(HostResumeResponse {
        resume: xml_child_text::<C>(root, "resume")?.parse()?,
        rtsp_session_url: xml_child_text::<C>(root, "sessionUrl0")?.to_string(),
    })
}

async fn inner_launch_host<C: RequestClient>(
    instance: &MoonlightInstance,
    client: &mut C,
    https_hostport: &str,
    verb: &str,
    info: ClientInfo<'_>,
    request: ClientStreamRequest,
) -> Result<C::Text, ApiError<C::Error>> {
    // TODO: figure out negotiated width / height https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/nvstream/http/NvHTTP.java#L765
    let mut query_params = DynamicQueryParams::default();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    let launch_params = form_urlencoded::parse(instance.launch_url_query_parameters().as_bytes());
    for (name, value) in launch_params {
        query_params.push((name, value));
    }

    // TODO: don't alloc heap
    query_params.push(query_param_owned("appid", request.app_id.to_string()));
    // TODO: implement this
    // TODO: don't alloc heap
    // Using an FPS value over 60 causes SOPS to default to 720p60,
    // so force it to 0 to ensure the correct resolution is set. We
    // used to use 60 here but that locked the frame rate to 60 FPS
    // on GFE 3.20.3. We don't need this hack for Sunshine.
    query_params.push(query_param_owned(
        "mode",
        format!(
            "{}x{}x{}",
            request.mode_width, request.mode_height, request.mode_fps
        ),
    ));
    query_params.push(query_param("additionalStates", "1"));

    let mut ri_key_str_bytes = [0u8; 16 * 2];
    hex::encode_to_slice(request.ri_key, &mut ri_key_str_bytes).expect("encode ri key");
    query_params.push(query_param(
        "rikey",
        str::from_utf8(&ri_key_str_bytes).expect("valid ri key str"),
    ));

    let mut ri_key_id_str_bytes = [0u8; 12];
    write!(&mut ri_key_id_str_bytes[..], "{}", request.ri_key_id).expect("write ri key id");
    query_params.push(query_param(
        "rikeyid",
        str::from_utf8(&ri_key_id_str_bytes).expect("valid ri key id str"),
    ));

    query_params.push(query_param("localAudioPlayMode", "todo")); // TODO
    query_params.push(query_param("surroundAudioInfo", "todo")); // TODO
    query_params.push(query_param("remoteControllersBitmap", "todo")); // TODO
    query_params.push(query_param("gcmap", "todo")); // TODO
    query_params.push(query_param("gcpersist", "todo")); // TODO

    let response = client
        .send_https_request_text_response(https_hostport, verb, &query_params)
        .await
        .map_err(ApiError::RequestClient)?;

    Ok(response)
}
