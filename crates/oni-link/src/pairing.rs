//! What the desktop's QR code carries: its key, a one-time secret, its name and where to reach it.

use std::net::SocketAddr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::Error;
use crate::noise::PublicKey;

const SCHEME: &str = "oni-link://pair?";

#[derive(Clone, Debug, PartialEq)]
pub struct PairingCode {
    pub host: PublicKey,
    pub secret: [u8; 32],
    pub name: String,
    pub addresses: Vec<SocketAddr>,
}

impl PairingCode {
    pub fn to_uri(&self) -> String {
        let addresses: Vec<String> = self.addresses.iter().map(SocketAddr::to_string).collect();
        format!(
            "{SCHEME}v={}&k={}&s={}&n={}&a={}",
            crate::wire::PROTOCOL,
            self.host.to_base64(),
            URL_SAFE_NO_PAD.encode(self.secret),
            URL_SAFE_NO_PAD.encode(self.name.as_bytes()),
            addresses.join(",")
        )
    }

    pub fn parse(uri: &str) -> Result<Self, Error> {
        let query = uri.strip_prefix(SCHEME).ok_or(Error::BadCode("scheme"))?;
        let (mut host, mut secret, mut name, mut addresses, mut version) = (None, None, None, None, None);
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').ok_or(Error::BadCode("query"))?;
            match key {
                "v" => version = value.parse::<u16>().ok(),
                "k" => host = Some(PublicKey::from_base64(value)?),
                "s" => {
                    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| Error::BadCode("secret"))?;
                    secret = Some(<[u8; 32]>::try_from(bytes).map_err(|_| Error::BadCode("secret"))?);
                }
                "n" => {
                    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| Error::BadCode("name"))?;
                    name = Some(String::from_utf8(bytes).map_err(|_| Error::BadCode("name"))?);
                }
                "a" => {
                    let parsed: Result<Vec<SocketAddr>, _> =
                        value.split(',').filter(|a| !a.is_empty()).map(str::parse).collect();
                    addresses = Some(parsed.map_err(|_| Error::BadCode("address"))?);
                }
                _ => {}
            }
        }
        if version != Some(crate::wire::PROTOCOL) {
            return Err(Error::Version(version.unwrap_or(0)));
        }
        Ok(PairingCode {
            host: host.ok_or(Error::BadCode("key"))?,
            secret: secret.ok_or(Error::BadCode("secret"))?,
            name: name.unwrap_or_default(),
            addresses: addresses.unwrap_or_default(),
        })
    }
}
