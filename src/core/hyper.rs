use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use hyper::{Request, Response, Uri, body::Incoming};
use hyper_rustls::FixedServerNameResolver;
use hyper_util::client::legacy::Client as LegacyClient;
use hyper_util::rt::TokioIo;
use rustls_pki_types::ServerName;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tower_service::Service;

pub struct EmptyBody;

impl hyper::body::Body for EmptyBody {
    type Data = &'static [u8];
    type Error = std::convert::Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        Poll::Ready(None)
    }

    fn is_end_stream(&self) -> bool {
        true
    }
}

#[derive(Clone)]
pub struct ConnectorService {
    timeout_duration: Duration,
}

impl ConnectorService {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout_duration: Duration::from_millis(timeout_ms),
        }
    }
}

impl Service<Uri> for ConnectorService {
    type Response = TokioIo<TcpStream>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        let t_duration = self.timeout_duration;

        Box::pin(async move {
            let addr: SocketAddr = format!("{}:{}", uri.host().unwrap(), uri.port_u16().unwrap())
                .parse()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            let stream = timeout(t_duration, TcpStream::connect(addr))
                .await
                .map_err(|_| "connect timeout")?
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            stream.set_nodelay(true).ok();
            Ok(TokioIo::new(stream))
        })
    }
}

pub type MyHttpsConnector = hyper_rustls::HttpsConnector<ConnectorService>;
pub type MyHyperClient = LegacyClient<MyHttpsConnector, EmptyBody>;

pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub fn build_hyper_client(timeout_ms: u64, server_name: String) -> Option<MyHyperClient> {
    let connector = ConnectorService::new(timeout_ms);

    let resolver = FixedServerNameResolver::new(ServerName::try_from(server_name).ok()?);

    let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .with_server_name_resolver(resolver)
        .enable_http1()
        .wrap_connector(connector);

    let client = LegacyClient::builder(hyper_util::rt::TokioExecutor::new())
        .pool_max_idle_per_host(1)
        .pool_idle_timeout(Duration::from_secs(1))
        .build(https_connector);

    Some(client)
}

pub async fn send_request(
    client: &MyHyperClient,
    host: &str,
    uri: Uri,
    method: http::Method,
    timeout_ms: u64,
) -> Option<Response<Incoming>> {
    let req = Request::builder()
        .uri(uri)
        .method(method)
        .header("User-Agent", USER_AGENT)
        .header("Host", host)
        .body(EmptyBody)
        .ok()?;

    timeout(Duration::from_millis(timeout_ms), client.request(req))
        .await
        .ok()?
        .ok()
}

pub fn parse_url(url: &str) -> Option<(Uri, String, &'static str, String)> {
    let uri = url.parse::<Uri>().ok()?;
    let host = uri.host()?.to_string();
    let scheme = uri.scheme_str()?;
    let scheme = if scheme == "https" { "https" } else { "http" };
    let path = uri.path().to_string();
    Some((uri, host, scheme, path))
}