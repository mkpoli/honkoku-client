use crate::{Error, OcrEngine, ProgressHandler, Result};
use honkoku_core::{
    HonkokuClient,
    cache::{SharedStorage, blocking},
};
use honkoku_iiif::{Fetcher, ImageService, ImageVersion};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrLine {
    pub reading_order: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub confidence: f64,
    pub koji: String,
    pub plain: String,
    pub raw: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrPage {
    pub width: u32,
    pub height: u32,
    pub processed_width: u32,
    pub processed_height: u32,
    pub model: String,
    pub lines: Vec<OcrLine>,
    pub timings: BTreeMap<String, f64>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub created_at: String,
}
impl OcrPage {
    pub fn to_canvas(&mut self, width: u32, height: u32) -> Result<()> {
        if [self.width, self.height, width, height].contains(&0) {
            return Err(Error::Setup("image dimensions are missing".into()));
        }
        let sx = f64::from(width) / f64::from(self.width);
        let sy = f64::from(height) / f64::from(self.height);
        for line in &mut self.lines {
            line.x *= sx;
            line.width *= sx;
            line.y *= sy;
            line.height *= sy;
        }
        self.width = width;
        self.height = height;
        self.lines.sort_by_key(|line| line.reading_order);
        Ok(())
    }
    pub fn site_result(&self) -> Result<Value> {
        if self.width == 0 || self.height == 0 {
            return Err(Error::Setup("image dimensions are missing".into()));
        }
        let scale = (3500.0 / f64::from(self.width.max(self.height))).min(1.0);
        Ok(
            json!({"engine":"minna", "text":self.lines.iter().map(|line| line.koji.as_str()).collect::<Vec<_>>().join("\n"),
            "lines":self.lines.iter().map(|line| json!({"x":line.x*scale,"y":line.y*scale,"width":line.width*scale,"height":line.height*scale,
            "confidence":line.confidence,"classId":1,"readingOrder":line.reading_order,"raw":line.raw})).collect::<Vec<_>>() }),
        )
    }
}
fn full_width(info: &Value) -> Option<u32> {
    let width = info["width"].as_f64()?;
    let height = info["height"].as_f64()?;
    let mut scale = 1.0_f64;
    let profiles = std::iter::once(info).chain(info["profile"].as_array().into_iter().flatten());
    for limits in profiles {
        if let Some(n) = limits["maxWidth"].as_f64() {
            scale = scale.min(n / width);
        }
        if let Some(n) = limits["maxHeight"].as_f64() {
            scale = scale.min(n / height);
        }
        if let Some(n) = limits["maxArea"].as_f64() {
            scale = scale.min((n / (width * height)).sqrt());
        }
    }
    (scale < 1.0).then_some((width * scale).floor().max(1.0) as u32)
}
pub async fn run_page(
    engine: &impl OcrEngine,
    client: &HonkokuClient,
    storage: &SharedStorage,
    fetcher: &Fetcher,
    entry_id: &str,
    index: u32,
    progress: ProgressHandler,
) -> Result<OcrPage> {
    if entry_id.is_empty() || entry_id.contains('/') {
        return Err(Error::Setup("invalid entry ID".into()));
    }
    let entry: Value = client.document(&format!("entries/{entry_id}")).await?;
    let manifest_url = entry["manifestUrl"]
        .as_str()
        .ok_or_else(|| Error::Setup("manifest URL is missing".into()))?;
    fetcher.allow_url(manifest_url)?;
    let manifest = fetcher.get_manifest(manifest_url).await?;
    let canvas = manifest
        .canvases
        .get(index as usize)
        .ok_or_else(|| Error::Setup("page index is out of range".into()))?;
    let cached = if let Some(service) = &canvas.image_service {
        fetcher.allow_url(&service.id)?;
        let info: Value = serde_json::from_slice(&fetcher.info(service).await?.read().await?)?;
        let canonical = ImageService {
            id: info["id"]
                .as_str()
                .or_else(|| info["@id"].as_str())
                .unwrap_or(&service.id)
                .into(),
            version: if info["type"] == "ImageService3"
                || info["@context"].to_string().contains("/image/3/")
            {
                ImageVersion::V3
            } else {
                service.version
            },
            profile: service.profile.clone(),
        };
        fetcher.allow_url(&canonical.id)?;
        fetcher.full(&canonical, full_width(&info)).await?
    } else {
        let url = canvas
            .image_url
            .as_ref()
            .ok_or_else(|| Error::Setup("page image is missing".into()))?;
        fetcher.allow_url(url)?;
        fetcher.get(url).await?
    };
    // Hold a private snapshot so concurrent cache eviction cannot remove the OCR input.
    let image = tempfile::NamedTempFile::new()?;
    tokio::fs::write(image.path(), cached.read().await?).await?;
    let mut result = engine.process_page(image.path(), None, progress).await?;
    result.to_canvas(canvas.width, canvas.height)?;
    result.created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| Error::Setup(e.to_string()))?;
    let saved = result.clone();
    let page_id = format!("{entry_id}_{index}");
    blocking(storage, move |db| {
        db.put_ocr(&page_id, &saved.model, &saved)
    })
    .await?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn respects_all_service_limits() {
        assert_eq!(
            full_width(
                &json!({"width":6000,"height":4000,"maxWidth":4000,"profile":["level2",{"maxArea":6000000}]})
            ),
            Some(3000)
        );
        assert_eq!(
            full_width(&json!({"width":6000,"height":4000,"maxHeight":1000})),
            Some(1500)
        );
    }
    #[test]
    fn canvas_and_site_use_distinct_coordinate_spaces() -> Result<()> {
        let mut page: OcrPage = serde_json::from_value(
            json!({"width":3000,"height":2000,"processed_width":3000,"processed_height":2000,"model":"v18","lines":[{"reading_order":1,"x":1500,"y":200,"width":50,"height":1000,"confidence":0.95,"koji":"字（じ）","plain":"字じ","raw":"<ruby>字<rt>じ</rt></ruby>"}],"timings":{},"warnings":[]}),
        )?;
        page.to_canvas(6000, 4000)?;
        assert_eq!(page.lines[0].x, 3000.0);
        let site = page.site_result()?;
        assert_eq!(site["lines"][0]["x"], 1750.0);
        assert_eq!(site["lines"][0]["raw"], "<ruby>字<rt>じ</rt></ruby>");
        assert!(site.get("createdAt").is_none());
        Ok(())
    }
}
