use super::*;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// A receiver's server on a bare router, with `limits`.
async fn server(limits: Limits) -> (SocketAddr, CancellationToken) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let tls = crate::tls::server_config(&Identity::generate().unwrap()).unwrap();
    let app = Router::new().route("/", get(|| async { "ok" }));
    let shutdown = CancellationToken::new();
    tokio::spawn(serve(listener, TlsAcceptor::from(Arc::new(tls)), app, shutdown.clone(), limits));
    (addr, shutdown)
}

/// True when the server closes `stream` within `wait`.
async fn closed<S: tokio::io::AsyncRead + Unpin>(stream: &mut S, wait: Duration) -> bool {
    let mut byte = [0u8; 1];
    matches!(tokio::time::timeout(wait, stream.read(&mut byte)).await, Ok(Ok(0) | Err(_)))
}

#[tokio::test]
async fn past_the_cap_a_connection_is_closed_unheard() {
    let limits = Limits { handshake: Duration::from_secs(30), head: None, connections: 2 };
    let (addr, _shutdown) = server(limits).await;
    let mut first = TcpStream::connect(addr).await.unwrap();
    let _second = TcpStream::connect(addr).await.unwrap();
    let mut third = TcpStream::connect(addr).await.unwrap();
    assert!(closed(&mut third, Duration::from_secs(5)).await, "the third is closed at once");
    // Twin: a connection under the cap is kept through its handshake's time.
    assert!(!closed(&mut first, Duration::from_millis(300)).await);
}

/// A TLS connection that sends no request, on a server whose head timeout is `head`.
async fn silent_after_tls(head: Option<Duration>) -> bool {
    let (addr, _shutdown) = server(Limits { head, ..Limits::DEFAULT }).await;
    let tcp = TcpStream::connect(addr).await.unwrap();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(crate::tls::client_config(None)));
    let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let mut tls = connector.connect(name, tcp).await.unwrap();
    closed(&mut tls, Duration::from_secs(2)).await
}

#[tokio::test]
async fn a_connection_that_sends_no_request_is_closed() {
    assert!(silent_after_tls(Some(Duration::from_millis(300))).await);
    // Twin: without the timeout, as hyper without a timer runs it, the silent connection stays.
    assert!(!silent_after_tls(None).await);
}
