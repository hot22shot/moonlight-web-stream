use std::{fmt::Display, num::ParseIntError, str::FromStr, string::FromUtf8Error};

use reqwest::{Client, Url};
use roxmltree::{Document, Error, Node};
use thiserror::Error;
use url::{ParseError, UrlQuery, form_urlencoded::Serializer};
use uuid::{Uuid, fmt::Hyphenated};

use crate::{MoonlightInstance, pair::SALT_LENGTH};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("{0}")]
    UrlParseError(#[from] ParseError),
    #[error("the response is invalid xml")]
    ParseXmlError(#[from] Error),
    #[error("the returned xml doc has a non 200 status code")]
    InvalidXmlStatusCode,
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
    #[error("{0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("{0}")]
    ParseUuidError(#[from] uuid::Error),
    #[error("{0}")]
    ParseHexError(#[from] hex::FromHexError),
    #[error("{0}")]
    Utf8Error(#[from] FromUtf8Error),
}

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

impl ClientInfo<'_> {
    fn add_query_params(&self, params: &mut Serializer<'_, UrlQuery>) {
        params.append_pair("uniqueid", self.unique_id);

        let mut uuid_str_bytes = [0; Hyphenated::LENGTH];
        self.uuid.as_hyphenated().encode_lower(&mut uuid_str_bytes);
        let uuid_str = str::from_utf8(&uuid_str_bytes).expect("uuid string");

        params.append_pair("uuid", uuid_str);
    }
}

fn xml_child_text<'doc, 'node>(
    list_node: Node<'node, 'doc>,
    name: &'static str,
) -> Result<&'node str, ApiError>
where
    'node: 'doc,
{
    let node = list_node
        .children()
        .find(|node| node.tag_name().name() == name)
        .ok_or(ApiError::DetailNotFound(name))?;
    let content = node.text().ok_or(ApiError::XmlTextNotFound(name))?;

    Ok(content)
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

fn build_url(
    use_https: bool,
    address: &str,
    path: &str,
    info: Option<ClientInfo<'_>>,
) -> Result<Url, ApiError> {
    let protocol = if use_https { "https" } else { "http" };
    let mut url = Url::parse(&format!("{protocol}://{address}/{path}"))?;

    if let Some(client_info) = info {
        let mut query_params = url.query_pairs_mut();
        client_info.add_query_params(&mut query_params);
    }

    Ok(url)
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
            "{}-{}-{}-{}",
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
    pub server_codec_mode_support: u32,
    pub pair_status: PairStatus,
    pub current_game: u32,
    pub state_string: String,
    pub state: ServerState,
}

pub async fn host_info(
    client: &Client,
    use_https: bool,
    address: &str,
    info: Option<ClientInfo<'_>>,
) -> Result<HostInfo, ApiError> {
    let url = build_url(use_https, address, "serverinfo", info)?;

    let response = client.get(url).send().await?.text().await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    let state_string = xml_child_text(root, "state")?.to_string();

    Ok(HostInfo {
        host_name: xml_child_text(root, "hostname")?.to_string(),
        app_version: xml_child_text(root, "appversion")?.parse()?,
        gfe_version: xml_child_text(root, "GfeVersion")?.to_string(),
        unique_id: xml_child_text(root, "uniqueid")?.parse()?,
        https_port: xml_child_text(root, "HttpsPort")?.parse()?,
        external_port: xml_child_text(root, "ExternalPort")?.parse()?,
        max_luma_pixels_hevc: xml_child_text(root, "MaxLumaPixelsHEVC")?.parse()?,
        mac: xml_child_text(root, "mac")?.to_string(),
        local_ip: xml_child_text(root, "LocalIP")?.to_string(),
        server_codec_mode_support: xml_child_text(root, "ServerCodecModeSupport")?.parse()?,
        pair_status: if xml_child_text(root, "PairStatus")?.parse::<u32>()? == 0 {
            PairStatus::NotPaired
        } else {
            PairStatus::Paired
        },
        current_game: xml_child_text(root, "currentgame")?.parse()?,
        state: ServerState::from_str(&state_string)?,
        state_string,
    })
}

// Pairing: https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/nvstream/http/PairingManager.java#L185

fn xml_child_paired<'doc, 'node>(
    list_node: Node<'node, 'doc>,
    name: &'static str,
) -> Result<PairStatus, ApiError>
where
    'node: 'doc,
{
    let content = xml_child_text(list_node, name)?.parse::<i32>()?;

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

pub async fn host_pair1(
    client: &Client,
    http_address: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest1<'_>,
) -> Result<HostPairResponse1, ApiError> {
    let mut url = build_url(false, http_address, "pair", Some(info))?;

    let mut query_params = url.query_pairs_mut();
    query_params.append_pair("devicename", request.device_name);
    query_params.append_pair("updateState", "1");

    // https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/nvstream/http/PairingManager.java#L207
    query_params.append_pair("phrase", "getservercert");

    let salt_str = hex::encode_upper(request.salt);
    query_params.append_pair("salt", &salt_str);

    let client_cert_pem_str = hex::encode_upper(request.client_cert_pem);
    query_params.append_pair("clientcert", &client_cert_pem_str);
    drop(query_params);

    let response = client.get(url).send().await?.text().await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    let paired = xml_child_paired(root, "paired")?;

    let cert = match xml_child_text(root, "plaincert") {
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

pub async fn host_pair2(
    client: &Client,
    http_address: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest2<'_>,
) -> Result<HostPairResponse2, ApiError> {
    let mut url = build_url(false, http_address, "pair", Some(info))?;

    let mut query_params = url.query_pairs_mut();

    query_params.append_pair("devicename", request.device_name);
    query_params.append_pair("updateState", "1");

    let encrypted_challenge_str = hex::encode_upper(request.encrypted_challenge);
    query_params.append_pair("clientchallenge", &encrypted_challenge_str);
    drop(query_params);

    let response = client.get(url).send().await?.text().await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    let paired = xml_child_paired(root, "paired")?;

    let challenge_response_str = xml_child_text(root, "challengeresponse")?;
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

pub async fn host_pair3(
    client: &Client,
    http_address: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest3<'_>,
) -> Result<HostPairResponse3, ApiError> {
    let mut url = build_url(false, http_address, "pair", Some(info))?;

    let mut query_params = url.query_pairs_mut();

    query_params.append_pair("devicename", request.device_name);
    query_params.append_pair("updateState", "1");

    let encrypted_challenge_str = hex::encode_upper(request.encrypted_challenge_response_hash);
    query_params.append_pair("serverchallengeresp", &encrypted_challenge_str);
    drop(query_params);

    let response = client.get(url).send().await?.text().await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    let paired = xml_child_paired(root, "paired")?;

    let pairing_secret_str = xml_child_text(root, "pairingsecret")?;
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

pub async fn host_pair4(
    client: &Client,
    http_address: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest4<'_>,
) -> Result<HostPairResponse4, ApiError> {
    let mut url = build_url(false, http_address, "pair", Some(info))?;

    let mut query_params = url.query_pairs_mut();

    query_params.append_pair("devicename", request.device_name);
    query_params.append_pair("updateState", "1");

    let client_pairing_secret_str = hex::encode_upper(request.client_pairing_secret);
    query_params.append_pair("clientpairingsecret", &client_pairing_secret_str);
    drop(query_params);

    let response = client.get(url).send().await?.text().await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    let paired = xml_child_paired(root, "paired")?;

    Ok(HostPairResponse4 { paired })
}

pub struct ClientPairRequest5<'a> {
    pub device_name: &'a str,
}
#[derive(Debug, Clone)]
pub struct ServerPairResponse5 {
    pub paired: PairStatus,
}

pub async fn host_pair5(
    client: &Client,
    http_address: &str,
    info: ClientInfo<'_>,
    request: ClientPairRequest5<'_>,
) -> Result<ServerPairResponse5, ApiError> {
    let mut url = build_url(false, http_address, "pair", Some(info))?;

    let mut query_params = url.query_pairs_mut();

    query_params.append_pair("phrase", "pairchallenge");
    query_params.append_pair("devicename", request.device_name);
    query_params.append_pair("updateState", "1");

    drop(query_params);

    let response = client.get(url).send().await?.text().await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    let paired = xml_child_paired(root, "paired")?;

    Ok(ServerPairResponse5 { paired })
}

pub async fn host_unpair(
    client: &Client,
    http_address: &str,
    info: ClientInfo<'_>,
) -> Result<(), ApiError> {
    let url = build_url(false, http_address, "unpair", Some(info))?;

    client.get(url).send().await?.text().await?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct App {
    pub id: u32,
    pub title: String,
    pub is_hdr_supported: bool,
}

#[derive(Debug, Clone)]
pub struct HostAppListResponse {
    pub apps: Vec<App>,
}

pub async fn host_app_list(
    client: &Client,
    https_address: &str,
    info: ClientInfo<'_>,
) -> Result<HostAppListResponse, ApiError> {
    let url = build_url(true, https_address, "applist", Some(info))?;

    let response = client.get(url).send().await?.text().await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    println!("{root:?}");

    todo!()
}

#[derive(Debug, Clone)]
pub struct ClientStreamRequest {
    pub app_id: u32,
    pub mode_width: u32,
    pub mode_height: u32,
    pub mode_fps: u32,
    pub ri_key: [u8; 16usize],
    pub ri_key_id: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct HostLaunchResponse {
    pub game_session: u32,
    pub rtsp_session_url: String,
}

pub async fn host_launch(
    instance: &MoonlightInstance,
    client: &Client,
    https_address: &str,
    info: ClientInfo<'_>,
    request: ClientStreamRequest,
) -> Result<HostLaunchResponse, ApiError> {
    let response =
        inner_launch_host(instance, client, https_address, "launch", info, request).await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    Ok(HostLaunchResponse {
        game_session: xml_child_text(root, "gamesession")?.parse()?,
        rtsp_session_url: xml_child_text(root, "sessionUrl0")?.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct HostResumeResponse {
    pub resume: u32,
    pub rtsp_session_url: String,
}

pub async fn host_resume(
    instance: &MoonlightInstance,
    client: &Client,
    https_address: &str,
    info: ClientInfo<'_>,
    request: ClientStreamRequest,
) -> Result<HostResumeResponse, ApiError> {
    let response =
        inner_launch_host(instance, client, https_address, "resume", info, request).await?;

    let doc = Document::parse(&response)?;
    let root = doc
        .root()
        .children()
        .find(|node| node.tag_name().name() == "root")
        .ok_or(ApiError::XmlRootNotFound)?;

    Ok(HostResumeResponse {
        resume: xml_child_text(root, "resume")?.parse()?,
        rtsp_session_url: xml_child_text(root, "sessionUrl0")?.to_string(),
    })
}

async fn inner_launch_host(
    instance: &MoonlightInstance,
    client: &Client,
    https_address: &str,
    verb: &str,
    info: ClientInfo<'_>,
    request: ClientStreamRequest,
) -> Result<String, ApiError> {
    let mut url = build_url(true, https_address, verb, Some(info))?;

    // TODO: figure out negotiated width / height https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/nvstream/http/NvHTTP.java#L765
    let mut query_params = url.query_pairs_mut();
    {
        let launch_params =
            form_urlencoded::parse(instance.launch_url_query_parameters().as_bytes());
        for (name, value) in launch_params {
            query_params.append_pair(&name, &value);
        }
    }

    query_params.append_pair("appid", &request.app_id.to_string());
    query_params.append_pair(
        "mode",
        &format!(
            "{}x{}x{}",
            request.mode_width, request.mode_height, request.mode_fps
        ),
    );
    query_params.append_pair("additionalStates", "1");
    query_params.append_pair("rikey", "todo"); // TODO
    query_params.append_pair("rikeyid", "todo"); // TODO
    query_params.append_pair("localAudioPlayMode", "todo"); // TODO
    query_params.append_pair("surroundAudioInfo", "todo"); // TODO
    query_params.append_pair("remoteControllersBitmap", "todo"); // TODO
    query_params.append_pair("gcmap", "todo"); // TODO
    query_params.append_pair("gcpersist", "todo"); // TODO
    drop(query_params);

    let response = client.get(url).send().await?.text().await?;

    Ok(response)
}
