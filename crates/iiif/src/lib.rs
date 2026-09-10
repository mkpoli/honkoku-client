//! IIIF Presentation normalization and Image API URL construction.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

mod cache;
mod fetcher;
mod protocol;
pub use bytes::Bytes;
pub use cache::{CacheEntry, Cached, DEFAULT_MAX_BYTES, ImageCache};
pub use fetcher::Fetcher;
pub use protocol::{local_url, rewrite_info_json, upstream_url};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid IIIF manifest: {0}")]
    Invalid(String),
    #[error("cache I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache index: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("host is not allowed: {0}")]
    Forbidden(String),
    #[error("request timed out")]
    Timeout,
    #[error("upstream returned HTTP {0}")]
    Status(u16),
    #[error("worker failed: {0}")]
    Worker(String),
    #[error("download exceeds the cache capacity")]
    TooLarge,
    #[error("shared request failed: {0}")]
    Shared(std::sync::Arc<Error>),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(pub BTreeMap<String, Vec<String>>);
impl Label {
    pub fn preferred<'a>(
        &'a self,
        languages: impl IntoIterator<Item = &'a str>,
    ) -> Option<&'a str> {
        languages
            .into_iter()
            .filter_map(|lang| self.0.get(lang))
            .find_map(|values| values.first())
            .or_else(|| self.0.get("none").and_then(|values| values.first()))
            .or_else(|| self.0.values().find_map(|values| values.first()))
            .map(String::as_str)
    }
    fn parse(value: &Value) -> Self {
        let mut label = Self::default();
        match value {
            Value::String(text) => {
                label.0.insert("none".into(), vec![text.clone()]);
            }
            Value::Array(values) => {
                for value in values {
                    for (lang, texts) in Self::parse(value).0 {
                        label.0.entry(lang).or_default().extend(texts);
                    }
                }
            }
            Value::Object(map) if map.contains_key("@value") => {
                if let Some(text) = map["@value"].as_str() {
                    label
                        .0
                        .entry(
                            map.get("@language")
                                .and_then(Value::as_str)
                                .unwrap_or("none")
                                .into(),
                        )
                        .or_default()
                        .push(text.into());
                }
            }
            Value::Object(map) => {
                for (lang, values) in map {
                    let texts = match values {
                        Value::String(text) => vec![text.clone()],
                        Value::Array(values) => values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect(),
                        _ => vec![],
                    };
                    label.0.insert(lang.clone(), texts);
                }
            }
            _ => {}
        }
        label
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageVersion {
    V1,
    V2,
    V3,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageService {
    pub id: String,
    pub version: ImageVersion,
    pub profile: Value,
}
impl ImageService {
    /// Parameters are Image API syntax. The service identifier is already encoded.
    pub fn url(
        &self,
        region: &str,
        size: &str,
        rotation: &str,
        quality: &str,
        format: &str,
    ) -> String {
        let size = match (self.version, size) {
            (ImageVersion::V3, "full") => "max",
            (ImageVersion::V1 | ImageVersion::V2, "max") => "full",
            _ => size,
        };
        let quality = match (self.version, quality) {
            (ImageVersion::V1, "default") => "native",
            (ImageVersion::V1, "gray") => "grey",
            (ImageVersion::V2 | ImageVersion::V3, "grey") => "gray",
            (ImageVersion::V2 | ImageVersion::V3, "native") => "default",
            _ => quality,
        };
        format!(
            "{}/{region}/{size}/{rotation}/{quality}.{format}",
            self.id.trim_end_matches('/')
        )
    }
    pub fn thumbnail(&self, width: u32) -> String {
        self.url("full", &format!("{},", width.max(1)), "0", "default", "jpg")
    }
    pub fn full(&self) -> String {
        self.url("full", "full", "0", "default", "jpg")
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Canvas {
    pub id: String,
    pub label: Label,
    pub width: u32,
    pub height: u32,
    pub image_service: Option<ImageService>,
    pub image_url: Option<String>,
    pub thumbnail_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub label: Label,
    pub canvases: Vec<Canvas>,
}
impl Manifest {
    pub fn parse(json: &str) -> Result<Self> {
        Self::from_value(&serde_json::from_str(json)?)
    }
    pub fn from_value(value: &Value) -> Result<Self> {
        let v3 = value["type"] == "Manifest";
        if !v3 && value["@type"] != "sc:Manifest" {
            return Err(Error::Invalid(
                "expected a Presentation 2 or 3 Manifest".into(),
            ));
        }
        let canvases = if v3 {
            value.get("items")
        } else {
            value.pointer("/sequences/0/canvases")
        }
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Invalid("missing canvases".into()))?;
        let canvases = canvases
            .iter()
            .map(|canvas| {
                let body = if v3 {
                    canvas["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .flat_map(|page| page["items"].as_array().into_iter().flatten())
                        .find(|annotation| annotation["motivation"] == "painting")
                        .and_then(|annotation| annotation.get("body"))
                } else {
                    canvas.pointer("/images/0/resource")
                };
                let body = body.map(first);
                let image_service = body.and_then(|body| body.get("service")).and_then(service);
                let image_url = body
                    .and_then(id)
                    .map(str::to_owned)
                    .or_else(|| image_service.as_ref().map(ImageService::full));
                let thumbnail_url = canvas
                    .get("thumbnail")
                    .and_then(image_id)
                    .or_else(|| {
                        body.and_then(|body| body.get("thumbnail"))
                            .and_then(image_id)
                    })
                    .or_else(|| image_service.as_ref().map(|service| service.thumbnail(200)));
                Ok(Canvas {
                    id: id(canvas)
                        .ok_or_else(|| Error::Invalid("canvas ID missing".into()))?
                        .into(),
                    label: Label::parse(&canvas["label"]),
                    width: dimension(canvas, "width")?,
                    height: dimension(canvas, "height")?,
                    image_service,
                    image_url,
                    thumbnail_url,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            label: Label::parse(&value["label"]),
            canvases,
        })
    }
}
fn dimension(value: &Value, key: &str) -> Result<u32> {
    value[key]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| Error::Invalid(format!("invalid canvas {key}")))
}
fn first(value: &Value) -> &Value {
    value
        .as_array()
        .and_then(|values| values.first())
        .unwrap_or(value)
}
fn id(value: &Value) -> Option<&str> {
    value
        .get("id")
        .or_else(|| value.get("@id"))
        .and_then(Value::as_str)
}
fn image_id(value: &Value) -> Option<String> {
    let value = first(value);
    value.as_str().or_else(|| id(value)).map(str::to_owned)
}
fn service(value: &Value) -> Option<ImageService> {
    if let Some(values) = value.as_array() {
        return values.iter().find_map(service);
    }
    let context = value["@context"].to_string();
    let profile = value["profile"].clone();
    let hint = format!("{context} {profile} {}", value["type"]);
    let version = if hint.contains("ImageService3") || hint.contains("/image/3/") {
        ImageVersion::V3
    } else if hint.contains("ImageService2") || hint.contains("/image/2/") {
        ImageVersion::V2
    } else if hint.contains("ImageService1")
        || hint.contains("/image/1/")
        || hint.contains("/image-api/1.1/")
        || hint.contains("/image-api/1.0/")
    {
        ImageVersion::V1
    } else {
        return None;
    };
    Some(ImageService {
        id: id(value)?.into(),
        version,
        profile,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_versions() -> Result<()> {
        let v2 = Manifest::parse(include_str!("../tests/v2.json"))?;
        assert_eq!(v2.label.preferred(["ja", "en"]), Some("資料"));
        assert_eq!(v2.label.0["en"], vec!["Manuscript"]);
        assert_eq!(
            v2.canvases[0].image_service.as_ref().map(|s| s.version),
            Some(ImageVersion::V2)
        );
        let v3 = Manifest::parse(include_str!("../tests/v3.json"))?;
        assert_eq!(v3.label.preferred(["ja", "en"]), Some("資料三"));
        assert_eq!(
            v3.canvases[0].image_service.as_ref().map(|s| s.full()),
            Some("https://images.example/abc/full/max/0/default.jpg".into())
        );
        assert!(v3.canvases[1].image_service.is_none());
        assert!(v3.canvases[1].image_url.is_none());
        Ok(())
    }
    #[test]
    fn urls_preserve_identifiers_and_version_syntax() {
        let base = "https://images.example/image/?IIIF=/17%2F170161%2F170161-0001.tif";
        for (version, size, quality) in [
            (ImageVersion::V1, "full", "native"),
            (ImageVersion::V2, "full", "default"),
            (ImageVersion::V3, "max", "default"),
        ] {
            let service = ImageService {
                id: base.into(),
                version,
                profile: Value::Null,
            };
            assert_eq!(
                service.full(),
                format!("{base}/full/{size}/0/{quality}.jpg")
            );
            assert_eq!(
                service.thumbnail(200),
                format!("{base}/full/200,/0/{quality}.jpg")
            );
            assert_eq!(
                service.url("10,20,30,40", "100,", "90", "gray", "png"),
                format!(
                    "{base}/10,20,30,40/100,/90/{}.png",
                    if version == ImageVersion::V1 {
                        "grey"
                    } else {
                        "gray"
                    }
                )
            );
        }
    }
}
