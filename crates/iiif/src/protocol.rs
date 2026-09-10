use crate::{Error, Result, fetcher::parse_url};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde_json::Value;

// JavaScript's encodeURIComponent leaves A-Z a-z 0-9 - _ . ! ~ * ' ( ) unchanged.
const COMPONENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

pub fn local_url(upstream: &str, is_windows: bool) -> String {
    let origin = if is_windows {
        "http://honkoku-iiif.localhost"
    } else {
        "honkoku-iiif://localhost"
    };
    format!(
        "{origin}/fetch?url={}",
        utf8_percent_encode(upstream, COMPONENT)
    )
}

/// Decode exactly once. OSD appends tile parameters to the rewritten service ID,
/// after the encoded query value; those parameters become part of the upstream URL.
pub fn upstream_url(path: &str, query: Option<&str>) -> Result<String> {
    if path != "/fetch" {
        return Err(Error::Invalid("unknown IIIF protocol path".into()));
    }
    let mut pairs = url::form_urlencoded::parse(query.unwrap_or_default().as_bytes());
    let (name, value) = pairs
        .next()
        .ok_or_else(|| Error::Invalid("missing url parameter".into()))?;
    if name != "url" || value.is_empty() || pairs.next().is_some() {
        return Err(Error::Invalid("expected one url parameter".into()));
    }
    parse_url(&value)?;
    Ok(value.into_owned())
}

/// Rewrites only the service identifier, preserving contexts, profiles and tile metadata.
pub fn rewrite_info_json(json: &Value, mut local: impl FnMut(&str) -> String) -> Result<Value> {
    let mut result = json.clone();
    let object = result
        .as_object_mut()
        .ok_or_else(|| Error::Invalid("info.json must be an object".into()))?;
    let mut found = false;
    for name in ["@id", "id"] {
        if let Some(value) = object.get_mut(name) {
            let upstream = value
                .as_str()
                .ok_or_else(|| Error::Invalid("invalid image service ID".into()))?;
            parse_url(upstream)?;
            *value = Value::String(local(upstream));
            found = true;
        }
    }
    if !found {
        return Err(Error::Invalid("info.json has no service ID".into()));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rewrites_image_versions_and_tile_roundtrips() -> Result<()> {
        let service = "https://images.example/image/?IIIF=/17%2F170161%2F170161-0001.tif";
        for (version, key) in [(1, "@id"), (2, "@id"), (3, "id")] {
            for windows in [true, false] {
                let original = json!({key: service, "@context": format!("http://iiif.io/api/image/{version}/context.json"),
                    "width": 1000, "height": 2000, "tiles": [{"width": 256, "scaleFactors": [1,2,4]}]});
                let result = rewrite_info_json(&original, |id| local_url(id, windows))?;
                assert_eq!(result["tiles"], original["tiles"]);
                assert_eq!(result["@context"], original["@context"]);
                let id = result[key]
                    .as_str()
                    .ok_or_else(|| Error::Invalid("test ID".into()))?;
                for suffix in ["/info.json", "/0,0,256,256/256,/0/default.jpg"] {
                    let request = url::Url::parse(&format!("{id}{suffix}"))
                        .map_err(|e| Error::Invalid(e.to_string()))?;
                    assert_eq!(
                        upstream_url(request.path(), request.query())?,
                        format!("{service}{suffix}")
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn encoding_and_invalid_requests() -> Result<()> {
        let upstream = "https://images.example/a+b%2Fc?x=1&y=日本語";
        let local = local_url(upstream, true);
        assert!(local.starts_with("http://honkoku-iiif.localhost/fetch?url=https%3A%2F%2F"));
        let parsed = url::Url::parse(&local).map_err(|e| Error::Invalid(e.to_string()))?;
        assert_eq!(upstream_url(parsed.path(), parsed.query())?, upstream);
        for query in [
            None,
            Some("url=file%3A%2F%2F%2Fetc%2Fpasswd"),
            Some("url=https://a&url=https://b"),
        ] {
            assert!(upstream_url("/fetch", query).is_err());
        }
        assert!(rewrite_info_json(&json!({}), |id| id.into()).is_err());
        Ok(())
    }
}
