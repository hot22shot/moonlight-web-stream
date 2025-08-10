use std::time::Duration;

use bytes::Bytes;
use pem::Pem;
use reqwest::{Certificate, Client, ClientBuilder, Identity};
use url::Url;

use crate::network::request_client::{QueryParamsRef, RequestClient};

fn default_builder() -> ClientBuilder {
    ClientBuilder::new()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(7))
}

fn build_url(
    use_https: bool,
    hostport: &str,
    path: &str,
    query_params: &QueryParamsRef<'_>,
) -> Result<Url, reqwest::Error> {
    let protocol = if use_https { "https" } else { "http" };

    let authority = format!("{protocol}://{hostport}/{path}");
    // TODO: remove unwrap
    let url = Url::parse_with_params(&authority, query_params).unwrap();

    Ok(url)
}

impl RequestClient for Client {
    type Error = reqwest::Error;

    type Text = String;
    type Bytes = Bytes;

    fn with_defaults() -> Result<Self, Self::Error> {
        default_builder().build()
    }

    fn with_certificates(
        client_private_key: &Pem,
        client_certificate: &Pem,
        server_certificate: &Pem,
    ) -> Result<Self, Self::Error> {
        let server_cert = Certificate::from_pem(server_certificate.to_string().as_bytes())?;

        let mut client_pem = String::new();
        client_pem.push_str(&client_private_key.to_string());
        client_pem.push('\n');
        client_pem.push_str(&client_certificate.to_string());

        let identity = Identity::from_pkcs8_pem(
            client_certificate.to_string().as_bytes(),
            client_private_key.to_string().as_bytes(),
        )?;

        default_builder()
            .use_native_tls()
            .tls_built_in_root_certs(false)
            .add_root_certificate(server_cert)
            .identity(identity)
            .danger_accept_invalid_hostnames(true)
            .build()
    }

    async fn send_http_request_text_response(
        &mut self,
        hostport: &str,
        path: &str,
        query_params: &QueryParamsRef<'_>,
    ) -> Result<Self::Text, Self::Error> {
        let url = build_url(false, hostport, path, query_params)?;
        self.get(url).send().await?.text().await
    }

    async fn send_https_request_text_response(
        &mut self,
        hostport: &str,
        path: &str,
        query_params: &QueryParamsRef<'_>,
    ) -> Result<Self::Text, Self::Error> {
        let url = build_url(true, hostport, path, query_params)?;
        self.get(url).send().await?.text().await
    }

    async fn send_https_request_data_response(
        &mut self,
        hostport: &str,
        path: &str,
        query_params: &QueryParamsRef<'_>,
    ) -> Result<Self::Bytes, Self::Error> {
        let url = build_url(true, hostport, path, query_params)?;
        self.get(url).send().await?.bytes().await
    }
}
