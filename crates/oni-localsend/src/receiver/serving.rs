//! The receiver's server loop: TLS, then HTTP/1.1, each connection bounded in number and in time.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ConnectInfo;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

/// A peer's TLS handshake, then each request's head, must arrive within this, so a silent
/// connection, or one sending a byte at a time, cannot hold a descriptor for ever.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections open at once; past it a new one is closed unheard.
const CONNECTIONS: usize = 32;

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub(super) handshake: Duration,
    pub(super) head: Option<Duration>,
    pub(super) connections: usize,
}

impl Limits {
    pub(super) const DEFAULT: Limits = Limits { handshake: HANDSHAKE_TIMEOUT, head: Some(HANDSHAKE_TIMEOUT), connections: CONNECTIONS };
}

pub(super) async fn serve(listener: TcpListener, acceptor: TlsAcceptor, app: Router, shutdown: CancellationToken, limits: Limits) {
    let connections = tokio_util::task::TaskTracker::new();
    let open = Arc::new(Semaphore::new(limits.connections));
    loop {
        let (tcp, peer) = tokio::select! {
            _ = shutdown.cancelled() => break,
            accepted = listener.accept() => match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::warn!("accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            },
        };
        let Ok(permit) = open.clone().try_acquire_owned() else { continue };
        let (acceptor, app, shutdown) = (acceptor.clone(), app.clone(), shutdown.clone());
        connections.spawn(async move {
            let _permit = permit;
            let Ok(Ok(tls)) = tokio::time::timeout(limits.handshake, acceptor.accept(tcp)).await else { return };
            let service = hyper::service::service_fn(move |mut req: axum::http::Request<hyper::body::Incoming>| {
                req.extensions_mut().insert(ConnectInfo(peer));
                app.clone().oneshot(req)
            });
            // hyper drops its head timeout without a timer to run it.
            let conn = hyper::server::conn::http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(limits.head)
                .serve_connection(TokioIo::new(tls), service);
            tokio::select! {
                _ = conn => {}
                _ = shutdown.cancelled() => {}
            }
        });
    }
    connections.close();
    connections.wait().await;
}
