use crate::Rpc;
use comsrv_protocol::{Request, Response};
use async_trait::async_trait;
use std::time::Duration;
use reqwest::{Client, Error};


impl From<reqwest::Error> for crate::Error {
    fn from(x: Error) -> Self {
        if x.is_decode() {
            crate::Error::UnexpectdResponse
        } else if x.is_timeout() {
            crate::Error::Timeout
        } else {
            crate::Error::Other(x.to_string())
        }
    }
}

#[derive(Clone)]
pub struct HttpRpc {
    host: String,
    port: u16,
    client: Option<reqwest::Client>,
}

impl HttpRpc {
    pub fn new() -> Self {
        HttpRpc {
            host: "127.0.0.1".to_string(),
            port: 5903,
            client: None
        }
    }

    pub fn with_host(host: &str) -> Self {
        HttpRpc {
            host: host.to_string(),
            port: 5903,
            client: None
        }
    }

    pub fn with_host_and_port(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            client: None
        }
    }
}

#[async_trait]
impl Rpc for HttpRpc {
    async fn request(&mut self, request: Request, timeout: Duration) -> crate::Result<Response> {
        let url = format!("{}:{}", self.host, self.port);
        let client = self.client.take().unwrap_or_else(|| Client::new());
        let response = client.get(&url).timeout(timeout).json(&request).send().await?;
        let ret = response.json::<Response>().await;
        if ret.is_ok() {
            self.client.replace(client);
        }
        ret.map_err(|x| x.into())
    }
}