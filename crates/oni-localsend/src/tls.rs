//! TLS on both sides: the receiver presents its identity, the client accepts any self-signed peer
//! unless a fingerprint is pinned.

use std::sync::Arc;
use std::time::Duration;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};

use crate::identity::{Identity, fingerprint};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

pub(crate) fn server_config(identity: &Identity) -> Result<rustls::ServerConfig, rustls::Error> {
    let mut config = rustls::ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![identity.cert_der()], identity.key_der())?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

/// An HTTPS client for LocalSend peers. LocalSend never verifies the chain; with `expected_fingerprint`
/// the handshake fails unless the presented certificate hashes to it.
pub fn https_client(expected_fingerprint: Option<&str>) -> Result<reqwest::Client, reqwest::Error> {
    let tls = client_config(expected_fingerprint);
    reqwest::Client::builder().tls_backend_preconfigured(tls).connect_timeout(CONNECT_TIMEOUT).no_proxy().build()
}

pub(crate) fn client_config(expected_fingerprint: Option<&str>) -> rustls::ClientConfig {
    let provider = provider();
    let verifier = PeerVerifier {
        expected: expected_fingerprint.map(str::to_ascii_lowercase),
        algorithms: provider.signature_verification_algorithms,
    };
    rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring supports the default versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth()
}

/// Whether an error chain carries a pinned-fingerprint refusal.
pub(crate) fn is_fingerprint_mismatch(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut cur = Some(err);
    while let Some(e) = cur {
        if matches!(
            e.downcast_ref::<rustls::Error>(),
            Some(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure))
        ) {
            return true;
        }
        // An io::Error's `source` skips the error it wraps, so step into it directly.
        let wrapped = e.downcast_ref::<std::io::Error>().and_then(|io| io.get_ref());
        cur = match wrapped {
            Some(inner) => Some(inner as &(dyn std::error::Error + 'static)),
            None => e.source(),
        };
    }
    false
}

#[derive(Debug)]
struct PeerVerifier {
    expected: Option<String>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for PeerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match &self.expected {
            Some(expected) if *expected != fingerprint(end_entity) => {
                Err(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure))
            }
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    // The handshake signatures stay verified: they prove the peer holds the pinned certificate's key.
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}
