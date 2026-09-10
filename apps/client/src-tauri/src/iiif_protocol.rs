use crate::{AppError, AppState};
use futures_util::{StreamExt, TryStreamExt, stream};
use honkoku_iiif::{
    Canvas, DEFAULT_MAX_BYTES, Error, Fetcher, ImageCache, ImageService, ImageVersion,
};
use serde::Serialize;
use tauri::{
    Manager, State,
    http::{Request, Response, StatusCode, header},
};

fn local_url(url: &str) -> String {
    honkoku_iiif::local_url(url, cfg!(target_os = "windows"))
}

pub fn initialize(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let cache = ImageCache::open(app.path().app_cache_dir()?.join("iiif"), DEFAULT_MAX_BYTES)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    app.manage(Fetcher::new(cache, client));
    Ok(())
}

pub fn handle(
    context: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let fetcher = context
        .app_handle()
        .try_state::<Fetcher>()
        .map(|state| state.inner().clone());
    tauri::async_runtime::spawn(async move {
        let response = match fetcher {
            Some(fetcher) => respond(&fetcher, request).await,
            None => text_response(StatusCode::SERVICE_UNAVAILABLE, "IIIF cache is unavailable"),
        };
        responder.respond(response);
    });
}

async fn respond(fetcher: &Fetcher, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() == tauri::http::Method::OPTIONS {
        let mut response = response(StatusCode::NO_CONTENT, Vec::new(), "text/plain");
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            header::HeaderValue::from_static("GET, HEAD, OPTIONS"),
        );
        return response;
    }
    if request.method() != tauri::http::Method::GET && request.method() != tauri::http::Method::HEAD
    {
        return text_response(StatusCode::METHOD_NOT_ALLOWED, "Use GET or HEAD");
    }
    let upstream = match honkoku_iiif::upstream_url(request.uri().path(), request.uri().query()) {
        Ok(url) => url,
        Err(_) => return text_response(StatusCode::BAD_REQUEST, "Invalid IIIF URL"),
    };
    let result = async {
        let cached = fetcher.get(&upstream).await?;
        let bytes = cached.read().await?;
        let (body, content_type) = if upstream.ends_with("/info.json") {
            let json = serde_json::from_slice(&bytes)?;
            register_info_hosts(fetcher, &json)?;
            (
                serde_json::to_vec(&honkoku_iiif::rewrite_info_json(&json, local_url)?)?,
                "application/json".into(),
            )
        } else {
            (bytes.to_vec(), cached.content_type)
        };
        Ok::<_, Error>((body, content_type))
    }
    .await;
    match result {
        Ok((body, content_type)) => {
            let mut response = response(StatusCode::OK, body, &content_type);
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("max-age=604800"),
            );
            if request.method() == tauri::http::Method::HEAD {
                if let Ok(length) =
                    header::HeaderValue::from_str(&response.body().len().to_string())
                {
                    response
                        .headers_mut()
                        .insert(header::CONTENT_LENGTH, length);
                }
                response.body_mut().clear();
            }
            response
        }
        Err(error) => {
            let (status, message) = protocol_error(&error);
            text_response(status, message)
        }
    }
}

fn response(status: StatusCode, body: Vec<u8>, content_type: &str) -> Response<Vec<u8>> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        header::HeaderValue::from_static("*"),
    );
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_str(content_type)
            .unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream")),
    );
    response
}

fn text_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    let mut response = response(
        status,
        message.as_bytes().to_vec(),
        "text/plain; charset=utf-8",
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

fn protocol_error(error: &Error) -> (StatusCode, &'static str) {
    match error {
        Error::Shared(error) => protocol_error(error),
        Error::Forbidden(_) => (StatusCode::FORBIDDEN, "IIIF host is not registered"),
        Error::Invalid(_) => (StatusCode::BAD_REQUEST, "Invalid IIIF data or URL"),
        Error::Timeout => (StatusCode::GATEWAY_TIMEOUT, "IIIF request timed out"),
        Error::Http(error) if error.is_timeout() => {
            (StatusCode::GATEWAY_TIMEOUT, "IIIF request timed out")
        }
        Error::Status(404) => (StatusCode::NOT_FOUND, "IIIF image was not found"),
        Error::Status(401 | 403) => (StatusCode::FORBIDDEN, "Upstream denied access"),
        Error::TooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            "IIIF image exceeds cache capacity",
        ),
        Error::Http(_) | Error::Status(_) | Error::Json(_) => {
            (StatusCode::BAD_GATEWAY, "IIIF upstream request failed")
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "IIIF cache failed"),
    }
}

fn command_error(error: Error) -> AppError {
    let (status, message) = protocol_error(&error);
    AppError {
        kind: format!("iiif_{}", status.as_u16()),
        message: message.into(),
    }
}

fn register_info_hosts(fetcher: &Fetcher, json: &serde_json::Value) -> honkoku_iiif::Result<()> {
    for key in ["@id", "id"] {
        if let Some(id) = json[key].as_str() {
            fetcher.allow_url(id)?;
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedCanvas {
    id: String,
    width: u32,
    height: u32,
    info_json_url: Option<String>,
    image_url: Option<String>,
    thumbnail_url: Option<String>,
}

#[tauri::command]
pub fn iiif_local_url(url: String, fetcher: State<'_, Fetcher>) -> Result<String, AppError> {
    fetcher.allow_url(&url).map_err(command_error)?;
    Ok(local_url(&url))
}

#[tauri::command]
pub async fn iiif_prepare_entry(
    entry_id: String,
    state: State<'_, AppState>,
    fetcher: State<'_, Fetcher>,
) -> Result<Vec<PreparedCanvas>, AppError> {
    let entry = state
        .client
        .cached_entry(&state.storage, &entry_id, false)
        .await?;
    fetcher
        .allow_url(&entry.manifest_url)
        .map_err(command_error)?;
    let manifest = fetcher
        .get_manifest(&entry.manifest_url)
        .await
        .map_err(command_error)?;
    // Register all discovered hosts before starting any prefetches or redirect chains.
    for canvas in &manifest.canvases {
        if let Some(service) = &canvas.image_service {
            fetcher.allow_url(&service.id).map_err(command_error)?;
        }
        for url in [&canvas.image_url, &canvas.thumbnail_url]
            .into_iter()
            .flatten()
        {
            fetcher.allow_url(url).map_err(command_error)?;
        }
    }
    stream::iter(manifest.canvases)
        .map(|canvas| prepare_canvas(fetcher.inner().clone(), canvas))
        .buffered(4)
        .try_collect()
        .await
        .map_err(command_error)
}

async fn prepare_canvas(fetcher: Fetcher, canvas: Canvas) -> honkoku_iiif::Result<PreparedCanvas> {
    let mut prepared = PreparedCanvas {
        id: canvas.id,
        width: canvas.width,
        height: canvas.height,
        info_json_url: None,
        image_url: canvas.image_url.as_deref().map(local_url),
        thumbnail_url: canvas.thumbnail_url.as_deref().map(local_url),
    };
    if let Some(service) = canvas.image_service {
        let info = fetcher.info(&service).await?;
        let json: serde_json::Value = serde_json::from_slice(&info.read().await?)?;
        register_info_hosts(&fetcher, &json)?;
        honkoku_iiif::rewrite_info_json(&json, local_url)?;
        let canonical = ImageService {
            id: json["id"]
                .as_str()
                .or_else(|| json["@id"].as_str())
                .unwrap_or(&service.id)
                .into(),
            version: if json["type"] == "ImageService3"
                || json["@context"].to_string().contains("/image/3/")
            {
                ImageVersion::V3
            } else {
                service.version
            },
            profile: service.profile.clone(),
        };
        fetcher.thumbnail(&canonical, 160).await?;
        prepared.info_json_url = Some(local_url(&format!(
            "{}/info.json",
            service.id.trim_end_matches('/')
        )));
        prepared.image_url = Some(local_url(&canonical.full()));
        prepared.thumbnail_url = Some(local_url(&canonical.thumbnail(160)));
    } else if let Some(thumbnail) = canvas.thumbnail_url {
        fetcher.get(&thumbnail).await?;
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_info_and_tiles_pass_through_the_protocol() -> Result<(), Box<dyn std::error::Error>> {
        struct Temporary(std::path::PathBuf);
        impl Drop for Temporary {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = Temporary(std::env::temp_dir().join(format!(
            "honkoku-iiif-protocol-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        )));
        tauri::async_runtime::block_on(async {
            let cache = ImageCache::open(&directory.0, 4096)?;
            let service = "https://images.example/image/?IIIF=/17%2F170161%2F170161-0001.tif";
            let info_url = format!("{service}/info.json");
            let info = serde_json::json!({"@id": service, "@context": "http://iiif.io/api/image/2/context.json",
                "width": 1024, "height": 2048, "tiles": [{"width": 256, "scaleFactors": [1,2,4]}]});
            cache.put(
                &info_url,
                "application/json",
                honkoku_iiif::Bytes::from(serde_json::to_vec(&info)?),
                None,
                None,
            )?;
            let suffix = "/0,0,256,256/256,/0/default.jpg";
            cache.put(
                &format!("{service}{suffix}"),
                "image/jpeg",
                honkoku_iiif::Bytes::from_static(b"tile"),
                None,
                None,
            )?;
            let fetcher = Fetcher::new(
                cache,
                reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?,
            );
            fetcher.allow_host("images.example")?;
            let request = Request::builder()
                .uri(local_url(&info_url))
                .body(Vec::new())?;
            let response = respond(&fetcher, request).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "max-age=604800");
            assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
            let json: serde_json::Value = serde_json::from_slice(response.body())?;
            assert_eq!(json["@id"], local_url(service));
            assert_eq!(json["tiles"], info["tiles"]);
            let tile_url = format!(
                "{}{suffix}",
                json["@id"].as_str().ok_or("missing service ID")?
            );
            let response = respond(
                &fetcher,
                Request::builder().uri(&tile_url).body(Vec::new())?,
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
            assert_eq!(response.body(), b"tile");
            let response = respond(
                &fetcher,
                Request::builder()
                    .method("HEAD")
                    .uri(&tile_url)
                    .body(Vec::new())?,
            )
            .await;
            assert!(response.body().is_empty());
            assert_eq!(response.headers()[header::CONTENT_LENGTH], "4");
            let response = respond(
                &fetcher,
                Request::builder()
                    .uri(local_url("https://unregistered.example/a"))
                    .body(Vec::new())?,
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert_eq!(fetcher.request_count(), 0);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
    }

    #[test]
    fn error_responses_have_cors_and_do_not_cache_failures() {
        let response = text_response(StatusCode::FORBIDDEN, "IIIF host is not registered");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            protocol_error(&Error::Shared(std::sync::Arc::new(Error::Forbidden(
                "example".into()
            ))))
            .0,
            StatusCode::FORBIDDEN
        );
    }
}
