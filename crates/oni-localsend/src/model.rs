//! The JSON bodies of LocalSend protocol v2, field for field.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

/// Protocol version announced to peers (major.minor).
pub const PROTOCOL_VERSION: &str = "2.1";
/// Default HTTP and multicast port.
pub const DEFAULT_PORT: u16 = 53317;

fn default_port() -> u16 {
    DEFAULT_PORT
}

/// A device as it describes itself in `register`, `info` and `prepare-upload`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub alias: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub device_model: Option<String>,
    #[serde(default)]
    pub device_type: Option<DeviceType>,
    #[serde(default)]
    pub fingerprint: String,
    /// Absent from `info` and `register` responses: the caller already knows it.
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub protocol: Protocol,
    #[serde(default)]
    pub download: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Mobile,
    Desktop,
    Web,
    Headless,
    Server,
}

/// Unknown values read as `desktop`, as the spec asks.
impl<'de> Deserialize<'de> for DeviceType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "mobile" => Self::Mobile,
            "web" => Self::Web,
            "headless" => Self::Headless,
            "server" => Self::Server,
            _ => Self::Desktop,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Http,
    #[default]
    Https,
}

impl Protocol {
    pub fn scheme(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

/// The multicast datagram: a device info plus whether it asks for answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    #[serde(flatten)]
    pub info: DeviceInfo,
    #[serde(default, alias = "announcement")]
    pub announce: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub id: String,
    pub file_name: String,
    pub size: u64,
    pub file_type: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub metadata: Option<FileTimes>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTimes {
    #[serde(default)]
    pub modified: Option<String>,
    #[serde(default)]
    pub accessed: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareUploadRequest {
    pub info: DeviceInfo,
    pub files: HashMap<String, FileMeta>,
}

/// `files` maps each accepted file id to its upload token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadResponse {
    pub session_id: String,
    pub files: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_spec_examples() {
        let a: Announcement = serde_json::from_str(
            r#"{"alias":"Nice Orange","version":"2.0","deviceModel":"Samsung","deviceType":"mobile",
               "fingerprint":"random string","port":53317,"protocol":"https","download":true,"announce":true}"#,
        )
        .unwrap();
        assert!(a.announce);
        assert_eq!(a.info.device_type, Some(DeviceType::Mobile));

        let info: DeviceInfo = serde_json::from_str(
            r#"{"alias":"Nice Orange","version":"2.0","deviceModel":null,"deviceType":"fridge","fingerprint":"x"}"#,
        )
        .unwrap();
        assert_eq!(info.device_type, Some(DeviceType::Desktop));
        assert_eq!((info.port, info.protocol, info.download), (53317, Protocol::Https, false));

        let req: PrepareUploadRequest = serde_json::from_str(
            r#"{"info":{"alias":"a","version":"2.0","fingerprint":"f","port":1,"protocol":"http"},
               "files":{"id1":{"id":"id1","fileName":"my image.png","size":324242,"fileType":"image/jpeg",
               "sha256":null,"preview":null,"metadata":{"modified":"2021-01-01T12:34:56Z"}}}}"#,
        )
        .unwrap();
        assert_eq!(req.files["id1"].metadata.as_ref().unwrap().accessed, None);
    }

    #[test]
    fn writes_camel_case() {
        let r = PrepareUploadResponse { session_id: "s".into(), files: HashMap::new() };
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"sessionId":"s","files":{}}"#);
        let m = FileMeta {
            id: "i".into(),
            file_name: "n".into(),
            size: 1,
            file_type: "t".into(),
            sha256: None,
            preview: None,
            metadata: None,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert!(v.get("fileName").is_some() && v.get("fileType").is_some());
    }
}
