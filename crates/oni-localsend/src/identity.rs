//! The TLS identity: a self-signed certificate whose SHA-256 is the device fingerprint.

use std::fs;
use std::io;
use std::path::Path;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sha2::{Digest, Sha256};

const CERT_FILE: &str = "localsend-cert.pem";
const KEY_FILE: &str = "localsend-key.pem";

#[derive(Debug)]
pub struct Identity {
    cert_pem: String,
    key_pem: String,
    cert: CertificateDer<'static>,
    fingerprint: String,
}

impl Clone for Identity {
    fn clone(&self) -> Self {
        Self::from_pem(&self.cert_pem, &self.key_pem).expect("validated on construction")
    }
}

impl Identity {
    pub fn generate() -> io::Result<Self> {
        let key = rcgen::generate_simple_self_signed(vec!["oni".to_string()]).map_err(io::Error::other)?;
        Self::from_pem(&key.cert.pem(), &key.signing_key.serialize_pem())
    }

    pub fn from_pem(cert_pem: &str, key_pem: &str) -> io::Result<Self> {
        let cert = CertificateDer::from_pem_slice(cert_pem.as_bytes()).map_err(io::Error::other)?;
        PrivateKeyDer::from_pem_slice(key_pem.as_bytes()).map_err(io::Error::other)?;
        Ok(Self { cert_pem: cert_pem.into(), key_pem: key_pem.into(), fingerprint: fingerprint(&cert), cert })
    }

    /// Reads the identity stored in `dir`, or creates and stores one, so the fingerprint survives restarts.
    pub fn load_or_create(dir: &Path) -> io::Result<Self> {
        let (cert_path, key_path) = (dir.join(CERT_FILE), dir.join(KEY_FILE));
        if cert_path.exists() && key_path.exists() {
            return Self::from_pem(&fs::read_to_string(cert_path)?, &fs::read_to_string(key_path)?);
        }
        let identity = Self::generate()?;
        fs::create_dir_all(dir)?;
        write_private(&key_path, &identity.key_pem)?;
        fs::write(cert_path, &identity.cert_pem)?;
        Ok(identity)
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    pub fn key_pem(&self) -> &str {
        &self.key_pem
    }

    pub(crate) fn cert_der(&self) -> CertificateDer<'static> {
        self.cert.clone()
    }

    pub(crate) fn key_der(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::from_pem_slice(self.key_pem.as_bytes()).expect("validated on construction")
    }
}

/// Lowercase hex SHA-256 of the DER certificate, as LocalSend writes it; peers compare it case-insensitively.
pub fn fingerprint(cert_der: &[u8]) -> String {
    hex(&Sha256::digest(cert_der))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(unix)]
fn write_private(path: &Path, contents: &str) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?
        .write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_across_loads() {
        let dir = tempfile::tempdir().unwrap();
        let a = Identity::load_or_create(dir.path()).unwrap();
        let b = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint().len(), 64);
        assert_ne!(a.fingerprint(), Identity::generate().unwrap().fingerprint());
    }
}
