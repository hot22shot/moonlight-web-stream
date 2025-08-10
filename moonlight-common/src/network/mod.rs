use std::{
    borrow::Cow, fmt::Display, io::Write as _, num::ParseIntError, str::FromStr,
    string::FromUtf8Error,
};

use roxmltree::{Document, Error, Node};
use thiserror::Error;
use uuid::{Uuid, fmt::Hyphenated};

use crate::{
    MoonlightInstance,
    network::request_client::{
        DynamicQueryParams, LocalQueryParams, QueryBuilder, RequestClient, query_param,
        query_param_owned,
    },
    pair::SALT_LENGTH,
    stream::ServerCodeModeSupport,
};

#[derive(Debug, Error)]
pub enum ApiError<RequestError> {
    #[error("{0}")]
    RequestClient(RequestError),
    #[error("the response is invalid xml")]
    ParseXmlError(#[from] Error),
    #[error("the returned xml doc has a non 200 status code")]
    InvalidXmlStatusCode { message: Option<String> },
    #[error("the returned xml doc doesn't have the root node")]
    XmlRootNotFound,
    #[error("the text contents of an xml node aren't present")]
    XmlTextNotFound(&'static str),
    #[error("detail was not found")]
    DetailNotFound(&'static str),
    #[error("{0}")]
    ParseServerStateError(#[from] ParseServerStateError),
    #[error("{0}")]
    ParseServerVersionError(#[from] ParseServerVersionError),
    #[error("parsing server codec mode support")]
    ParseServerCodecModeSupport,
    #[error("{0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("{0}")]
    ParseUuidError(#[from] uuid::Error),
    #[error("{0}")]
    ParseHexError(#[from] hex::FromHexError),
    #[error("{0}")]
    Utf8Error(#[from] FromUtf8Error),
}

pub mod request_client;
mod reqwest;

pub const DEFAULT_UNIQUE_ID: &str = "0123456789ABCDEF";

#[derive(Debug, Clone, Copy)]
pub struct ClientInfo<'a> {
    /// It's recommended to use the same (default) UID for all Moonlight clients so we can quit games started by other Moonlight clients.
    pub unique_id: &'a str,
    pub uuid: Uuid,
}

impl Default for ClientInfo<'static> {
    fn default() -> Self {
        Self {
            unique_id: DEFAULT_UNIQUE_ID,
            uuid: Uuid::new_v4(),
        }
    }
}

impl<'a> ClientInfo<'a> {
    fn add_query_params(
        &self,
        uuid_bytes: &'a mut [u8; Hyphenated::LENGTH],
        query_params: &mut impl QueryBuilder<'a>,
    ) {
        query_params.push((Cow::Borrowed("uniqueid"), Cow::Borrowed(self.unique_id)));

        self.uuid.as_hyphenated().encode_lower(uuid_bytes);
        let uuid_str = str::from_utf8(uuid_bytes).expect("uuid string");

        query_params.push((Cow::Borrowed("uuid"), Cow::Borrowed(uuid_str)));
    }
}

fn xml_child_text<'doc, 'node, C: RequestClient>(
    list_node: Node<'node, 'doc>,
    name: &'static str,
) -> Result<&'node str, ApiError<C::Error>>
where
    'node: 'doc,
{
    let node = list_node
        .children()
        .find(|node| node.tag_name().name() == name)
        .ok_or(ApiError::<C::Error>::DetailNotFound(name))?;
    let content = node
        .text()
        .ok_or(ApiError::<C::Error>::XmlTextNotFound(name))?;

    Ok(content)
}

fn xml_root_node<'doc, C>(doc: &'doc Document) -> Result<Node<'doc, 'doc>, ApiError<C>> {
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    let status_code = root
        .attribute("status_code")
        .ok_or(ApiError::DetailNotFound("status_code"))?
        .parse::<u32>()?;

    if status_code / 100 == 4 {
        return Err(ApiError::InvalidXmlStatusCode {
            message: root.attribute("status_message").map(str::to_string),
        });
    }

    Ok(root)
}

#[derive(Debug, Error, Clone)]
#[error("failed to parse the state of the server")]
pub struct ParseServerStateError;

#[derive(Debug, Copy, Clone)]
pub enum ServerState {
    Busy,
    Free,
}

impl FromStr for ServerState {
    type Err = ParseServerStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.ends_with("FREE") => Ok(ServerState::Free),
            s if s.ends_with("BUSY") => Ok(ServerState::Busy),
            _ => Err(ParseServerStateError),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PairStatus {
    NotPaired,
    Paired,
}

#[derive(Debug, Error)]
#[error("failed to parse server version")]
pub enum ParseServerVersionError {
    #[error("{0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("invalid version pattern")]
    InvalidPattern,
}

#[derive(Debug, Clone, Copy)]
pub struct ServerVersion {
    // TODO: what are those?
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
    pub mini_patch: i32,
}

impl Display for ServerVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.mini_patch
        )
    }
}

impl FromStr for ServerVersion {
    type Err = ParseServerVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.splitn(4, ".");

        let major = split
            .next()
            .ok_or(ParseServerVersionError::InvalidPattern)?
            .parse()?;
        let minor = split
            .next()
            .ok_or(ParseServerVersionError::InvalidPattern)?
            .parse()?;
        let patch = split
            .next()
            .ok_or(ParseServerVersionError::InvalidPattern)?
            .parse()?;
        let mini_patch = split
            .next()
            .ok_or(ParseServerVersionError::InvalidPattern)?
            .parse()?;

        Ok(Self {
            major,
            minor,
            patch,
            mini_patch,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub host_name: String,
    pub app_version: ServerVersion,
    pub gfe_version: String,
    pub unique_id: Uuid,
    pub https_port: u16,
    pub external_port: u16,
    pub max_luma_pixels_hevc: u32,
    pub mac: String,
    pub local_ip: String,
    pub server_codec_mode_support: ServerCodeModeSupport,
    pub pair_status: PairStatus,
    pub current_game: u32,
    pub state_string: String,
    pub state: ServerState,
}

pub async fn host_info<C: RequestClient>(
    client: &mut C,
    use_https: bool,
    hostport: &str,
    info: Option<ClientInfo<'_>>,
) -> Result<HostInfo, ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<2>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    if let Some(info) = info {
        info.add_query_params(&mut uuid_bytes, &mut query_params);
    }

    let response = if use_https {
        client
            .send_https_request_text_response(hostport, "serverinfo", &query_params)
            .await
            .map_err(|err| ApiError::RequestClient(err))?
    } else {
        client
            .send_http_request_text_response(hostport, "serverinfo", &query_params)
            .await
            .map_err(|err| ApiError::RequestClient(err))?
    };

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    let state_string = xml_child_text::<C>(root, "state")?.to_string();

    Ok(HostInfo {
        host_name: xml_child_text::<C>(root, "hostname")?.to_string(),
        app_version: xml_child_text::<C>(root, "appversion")?.parse()?,
        gfe_version: xml_child_text::<C>(root, "GfeVersion")?.to_string(),
        unique_id: xml_child_text::<C>(root, "uniqueid")?.parse()?,
        https_port: xml_child_text::<C>(root, "HttpsPort")?.parse()?,
        external_port: xml_child_text::<C>(root, "ExternalPort")?.parse()?,
        max_luma_pixels_hevc: xml_child_text::<C>(root, "MaxLumaPixelsHEVC")?.parse()?,
        mac: xml_child_text::<C>(root, "mac")?.to_string(),
        local_ip: xml_child_text::<C>(root, "LocalIP")?.to_string(),
        server_codec_mode_support: ServerCodeModeSupport::from_bits(
            xml_child_text::<C>(root, "ServerCodecModeSupport")?.parse()?,
        )
        .ok_or(ApiError::ParseServerCodecModeSupport)?,
        pair_status: if xml_child_text::<C>(root, "PairStatus")?.parse::<u32>()? == 0 {
            PairStatus::NotPaired
        } else {
            PairStatus::Paired
        },
        current_game: xml_child_text::<C>(root, "currentgame")?.parse()?,
        state: ServerState::from_str(&state_string)?,
        state_string,
    })
}

// Pairing: https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/nvstream/http/PairingManager.java#L185

fn xml_child_paired<'doc, 'node, C: RequestClient>(
    list_node: Node<'node, 'doc>,
    name: &'static str,
) -> Result<PairStatus, ApiError<C::Error>>
where
    'node: 'doc,
{
    let content = xml_child_text::<C>(list_node, name)?.parse::<i32>()?;

    Ok(if content == 1 {
        PairStatus::Paired
    } else {
        PairStatus::NotPaired
    })
}

#[derive(Debug, Clone)]
pub struct ClientPairRequest1<'a> {
    pub device_name: &'a str,
    pub salt: [u8; SALT_LENGTH],
    pub client_cert_pem: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct HostPairResponse1 {
    pub paired: PairStatus,
    pub cert: Option<String>,
}

pub async fn host_pair1<C: RequestClient>(
    client: &mut C,
    http_hostport: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest1<'_>,
) -> Result<HostPairResponse1, ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<{ 2 + 7 }>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    query_params.push(query_param("devicename", request.device_name));
    query_params.push(query_param("updateState", "1"));

    // https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/nvstream/http/PairingManager.java#L207
    query_params.push(query_param("phrase", "getservercert"));

    let salt_str = hex::encode_upper(request.salt);
    query_params.push(query_param("salt", &salt_str));

    let client_cert_pem_str = hex::encode_upper(request.client_cert_pem);
    query_params.push(query_param("clientcert", &client_cert_pem_str));

    let response = client
        .send_http_request_text_response(http_hostport, "pair", &query_params)
        .await
        .map_err(|err| ApiError::RequestClient(err))?;

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    let paired = xml_child_paired::<C>(root, "paired")?;

    let cert = match xml_child_text::<C>(root, "plaincert") {
        Ok(value) => {
            let value = hex::decode(value)?;

            Some(String::from_utf8(value)?)
        }
        Err(ApiError::DetailNotFound("plaincert")) => None,
        Err(err) => return Err(err),
    };

    Ok(HostPairResponse1 { paired, cert })
}

#[derive(Debug, Clone)]
pub struct ClientPairRequest2<'a> {
    pub device_name: &'a str,
    pub encrypted_challenge: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct HostPairResponse2 {
    pub paired: PairStatus,
    pub encrypted_response: Vec<u8>,
}

pub async fn host_pair2<C: RequestClient>(
    client: &mut C,
    http_hostport: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest2<'_>,
) -> Result<HostPairResponse2, ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<{ 2 + 3 }>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    query_params.push(query_param("devicename", request.device_name));
    query_params.push(query_param("updateState", "1"));

    let encrypted_challenge_str = hex::encode_upper(request.encrypted_challenge);
    query_params.push(query_param("clientchallenge", &encrypted_challenge_str));

    let response = client
        .send_http_request_text_response(http_hostport, "pair", &query_params)
        .await
        .map_err(|err| ApiError::RequestClient(err))?;

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    let paired = xml_child_paired::<C>(root, "paired")?;

    let challenge_response_str = xml_child_text::<C>(root, "challengeresponse")?;
    let challenge_response = hex::decode(challenge_response_str)?;

    Ok(HostPairResponse2 {
        paired,
        encrypted_response: challenge_response,
    })
}

pub struct ClientPairRequest3<'a> {
    pub device_name: &'a str,
    pub encrypted_challenge_response_hash: &'a [u8],
}
#[derive(Debug, Clone)]
pub struct HostPairResponse3 {
    pub paired: PairStatus,
    pub server_pairing_secret: Vec<u8>,
}

pub async fn host_pair3<C: RequestClient>(
    client: &mut C,
    http_hostport: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest3<'_>,
) -> Result<HostPairResponse3, ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<{ 2 + 3 }>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    query_params.push(query_param("devicename", request.device_name));
    query_params.push(query_param("updateState", "1"));

    let encrypted_challenge_str = hex::encode_upper(request.encrypted_challenge_response_hash);
    query_params.push(query_param("serverchallengeresp", &encrypted_challenge_str));

    let response = client
        .send_http_request_text_response(http_hostport, "pair", &query_params)
        .await
        .map_err(|err| ApiError::RequestClient(err))?;

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    let paired = xml_child_paired::<C>(root, "paired")?;

    let pairing_secret_str = xml_child_text::<C>(root, "pairingsecret")?;
    let pairing_secret = hex::decode(pairing_secret_str)?;

    Ok(HostPairResponse3 {
        paired,
        server_pairing_secret: pairing_secret,
    })
}

pub struct ClientPairRequest4<'a> {
    pub device_name: &'a str,
    pub client_pairing_secret: &'a [u8],
}
#[derive(Debug, Clone)]
pub struct HostPairResponse4 {
    pub paired: PairStatus,
}

pub async fn host_pair4<C: RequestClient>(
    client: &mut C,
    http_hostport: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest4<'_>,
) -> Result<HostPairResponse4, ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<{ 2 + 3 }>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    query_params.push(query_param("devicename", request.device_name));
    query_params.push(query_param("updateState", "1"));

    let client_pairing_secret_str = hex::encode_upper(request.client_pairing_secret);
    query_params.push(query_param(
        "clientpairingsecret",
        &client_pairing_secret_str,
    ));

    let response = client
        .send_http_request_text_response(http_hostport, "pair", &query_params)
        .await
        .map_err(|err| ApiError::RequestClient(err))?;

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    let paired = xml_child_paired::<C>(root, "paired")?;

    Ok(HostPairResponse4 { paired })
}

pub struct ClientPairRequest5<'a> {
    pub device_name: &'a str,
}
#[derive(Debug, Clone)]
pub struct ServerPairResponse5 {
    pub paired: PairStatus,
}

pub async fn host_pair5<C: RequestClient>(
    client: &mut C,
    http_hostport: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest5<'_>,
) -> Result<ServerPairResponse5, ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<{ 2 + 3 }>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    query_params.push(query_param("phrase", "pairchallenge"));
    query_params.push(query_param("devicename", request.device_name));
    query_params.push(query_param("updateState", "1"));

    let response = client
        .send_http_request_text_response(http_hostport, "pair", &query_params)
        .await
        .map_err(|err| ApiError::RequestClient(err))?;

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    let paired = xml_child_paired::<C>(root, "paired")?;

    Ok(ServerPairResponse5 { paired })
}

pub async fn host_unpair<C: RequestClient>(
    client: &mut C,
    http_hostport: &str,
    info: ClientInfo<'_>,
) -> Result<(), ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<2>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    client.send_http_request_text_response(http_hostport, "unpair", &query_params);

    Ok(())
}

#[derive(Debug, Clone)]
pub struct App {
    pub id: u32,
    pub title: String,
    pub is_hdr_supported: bool,
}

#[derive(Debug, Clone)]
pub struct ServerAppListResponse {
    pub apps: Vec<App>,
}

pub async fn host_app_list<C: RequestClient>(
    client: &mut C,
    https_hostport: &str,
    info: ClientInfo<'_>,
) -> Result<ServerAppListResponse, ApiError<C::Error>> {
    let mut query_params = LocalQueryParams::<2>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    let response = client
        .send_https_request_text_response(https_hostport, "applist", &query_params)
        .await
        .map_err(|err| ApiError::RequestClient(err))?;

    let doc = Document::parse(response.as_ref())?;
    let root = xml_root_node(&doc)?;

    let apps = root
        .children()
        .filter(|node| node.tag_name().name() == "App")
        .map(|app_node| {
            let title = xml_child_text::<C>(app_node, "AppTitle")?.to_string();

            let id = xml_child_text::<C>(app_node, "ID")?.parse()?;

            let is_hdr_supported = xml_child_text::<C>(app_node, "IsHdrSupported")
                .unwrap_or("0")
                .parse::<u32>()?
                == 1;

            Ok(App {
                id,
                title,
                is_hdr_supported,
            })
        })
        .collect::<Result<Vec<_>, ApiError<_>>>()?;

    Ok(ServerAppListResponse { apps })
}

#[derive(Debug, Clone)]
pub struct ClientAppBoxArtRequest {
    pub app_id: u32,
}

pub async fn host_app_box_art<C: RequestClient>(
    client: &mut C,
    https_address: &str,
    info: ClientInfo<'_>,
    request: ClientAppBoxArtRequest,
) -> Result<C::Bytes, ApiError<C::Error>> {
    // Assets: https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/nvstream/http/NvHTTP.java#L721
    let mut query_params = LocalQueryParams::<{ 2 + 3 }>::new();

    let mut uuid_bytes = [0; Hyphenated::LENGTH];
    info.add_query_params(&mut uuid_bytes, &mut query_params);

    // TODO: don't format use array
    query_params.push(query_param_owned("appid", format!("{}", request.app_id)));
    query_params.push(query_param("AssetType", "2"));
    query_params.push(query_param("AssetIdx", "0"));

    let response = client
        .send_https_request_data_response(https_address, "appasset", &query_params)
        .await
        .map_err(|err| ApiError::RequestClient(err))?;

    Ok(response)
}

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
    let mut query_params = DynamicQueryParams::new();

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
        .map_err(|err| ApiError::RequestClient(err))?;

    Ok(response)
}
