use std::net::{IpAddr, SocketAddr};

use clap::{Args, Parser, Subcommand};
use common::config::{
    Config, ConfigSsl, ForwardedHeaders, PortRange, WebRtcNat1To1IceCandidateType,
    WebRtcNat1To1Mapping, WebRtcNetworkType,
};
use log::LevelFilter;

#[derive(Parser)]
#[command(version,about, long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value = "./server/config.json")]
    pub config_file: String,

    #[command(flatten)]
    pub options: CliConfig,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Runs the server (default if no command specified)
    Run,
    /// Prints the config into stdout in json format
    PrintConfig,
}

#[derive(Args)]
pub struct CliConfig {
    /// Overwrites `webrtc.port_range`. Specify like this: "MIN:MAX".
    #[arg(long, env = "WEBRTC_PORT_RANGE")]
    pub webrtc_port_range: Option<PortRange>,
    /// Overwrites `webrtc.nat_1to1.ice_candidate_type` to `host` and uses the ip address as the `webrtc.nat_1to1.ips`.
    #[arg(long, env = "WEBRTC_NAT_1TO1_HOST")]
    pub webrtc_nat_1to1_host: Option<IpAddr>,
    /// Overwrites `webrtc.network_types`. Example: "udp4,udp6"
    #[arg(long, env = "WEBRTC_NETWORK_TYPES", value_delimiter = ',')]
    pub webrtc_network_types: Option<Vec<WebRtcNetworkType>>,
    /// Overwrites `webrtc.include_loopback_candidates`.
    #[arg(long, env = "WEBRTC_INCLUDE_LOOPBACK_CANDIDATES")]
    pub webrtc_include_loopback_candidates: Option<bool>,
    /// Overwrites `web_server.bind_address`.
    #[arg(long, env = "BIND_ADDRESS")]
    pub bind_address: Option<SocketAddr>,
    /// Overwrites `web_server.certificate.certificate_pem`.
    #[arg(long, env = "SSL_CERTIFICATE")]
    pub ssl_certificate: Option<String>,
    /// Overwrites `web_server.certificate.private_key_pem`.
    #[arg(long, env = "SSL_PRIVATE_KEY")]
    pub ssl_private_key: Option<String>,
    /// Overwrites `web_server.forwarded_header.username_header`.
    #[arg(long, env = "FORWARDED_HEADER")]
    pub forwarded_header: Option<String>,
    /// Overwrites `log.level_filter`.
    #[arg(long, env = "LOG_LEVEL")]
    pub log_level_filter: Option<LevelFilter>,
    /// Overwrites `log.log_file_path`.
    #[arg(long, env = "LOG_FILE")]
    pub log_file: Option<String>,
}

impl CliConfig {
    pub fn apply(self, config: &mut Config) {
        if let Some(webrtc_port_range) = self.webrtc_port_range {
            config.webrtc.port_range = Some(webrtc_port_range);
        }
        if let Some(webrtc_nat_1to1_host) = self.webrtc_nat_1to1_host {
            config.webrtc.nat_1to1 = Some(WebRtcNat1To1Mapping {
                // TODO: test ip.to_string
                ips: vec![webrtc_nat_1to1_host.to_string()],
                ice_candidate_type: WebRtcNat1To1IceCandidateType::Host,
            });
        }
        if let Some(webrtc_network_types) = self.webrtc_network_types {
            config.webrtc.network_types = webrtc_network_types;
        }
        if let Some(webrtc_include_loopback_candidates) = self.webrtc_include_loopback_candidates {
            config.webrtc.include_loopback_candidates = webrtc_include_loopback_candidates;
        }
        if let Some(bind_address) = self.bind_address {
            config.web_server.bind_address = bind_address;
        }
        match (self.ssl_certificate, self.ssl_private_key) {
            (Some(certificate), Some(private_key)) => {
                config.web_server.certificate = Some(ConfigSsl {
                    certificate_pem: certificate,
                    private_key_pem: private_key,
                })
            }
            (None, None) => {}
            _ => {
                panic!("To enable https you need to set --ssl-certificate and --ssl-private-key");
            }
        }
        if let Some(forwarded_header) = self.forwarded_header {
            config.web_server.forwarded_header = Some(ForwardedHeaders {
                username_header: forwarded_header,
                auto_create_missing_user: config
                    .web_server
                    .forwarded_header
                    .clone()
                    .unwrap_or_default()
                    .auto_create_missing_user,
            });
        }
        if let Some(log_level_filter) = self.log_level_filter {
            config.log.level_filter = log_level_filter;
        }
        if let Some(log_file) = self.log_file {
            config.log.file_path = Some(log_file);
        }
    }
}
