//! Character crops with stored boxes from Glyph Atlas (glyphatlas.org).
use crate::{Error, Result, endpoint};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

pub const ATLAS_BASE: &str = "https://glyphatlas.org";
pub const ATLAS_LIMIT: usize = 200;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasGlyphs {
    pub character: String,
    pub code_point: String,
    pub name: Option<String>,
    pub readings: Vec<String>,
    /// 字母 of a hentaigana.
    pub jibo: Vec<String>,
    pub variants: Vec<AtlasVariant>,
    /// Credit lines of the variant tables, keyed by table name.
    pub variant_sources: BTreeMap<String, String>,
    pub total: u64,
    pub items: Vec<AtlasGlyph>,
    pub page_url: String,
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasVariant {
    pub character: String,
    pub count: u64,
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasGlyph {
    pub id: String,
    pub label: String,
    pub image: String,
    pub source: String,
    pub page_number: Option<u64>,
    /// みんなで翻刻 entry and 0-based page, when the record names one.
    pub entry_id: Option<String>,
    pub page_index: Option<u64>,
    pub licence: String,
    pub holder: Option<String>,
    pub attribution: Option<String>,
    pub rights_url: Option<String>,
    pub state: Option<String>,
    pub record_url: String,
}

#[derive(Deserialize)]
struct Named {
    char: String,
}
#[derive(Deserialize)]
struct VariantItem {
    char: String,
    #[serde(default)]
    corpus_count: u64,
}
#[derive(Deserialize, Default)]
struct Variants {
    #[serde(default)]
    items: Vec<VariantItem>,
    #[serde(default)]
    related: Vec<Named>,
    #[serde(default)]
    sources: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Detail {
    char: String,
    code_point: String,
    name: Option<String>,
    #[serde(default)]
    readings: Vec<String>,
    #[serde(default)]
    jibo: Vec<Named>,
    #[serde(default)]
    variants: Variants,
}
#[derive(Deserialize)]
struct Occurrences {
    total: u64,
    #[serde(default)]
    items: Vec<Occurrence>,
}
#[derive(Deserialize)]
struct Occurrence {
    id: String,
    label: String,
    image: String,
    #[serde(default)]
    source: String,
    page_number: Option<u64>,
    document: Option<String>,
    licence: String,
    holder: Option<String>,
    attribution: Option<String>,
    rights_url: Option<String>,
    state: Option<String>,
}

/// One grapheme's detail and up to `limit` of its crops, exact character scope.
pub async fn glyphs(character: &str, limit: usize) -> Result<AtlasGlyphs> {
    glyphs_at(ATLAS_BASE, character, limit).await
}
async fn glyphs_at(base: &str, character: &str, limit: usize) -> Result<AtlasGlyphs> {
    if character.is_empty() || !(1..=ATLAS_LIMIT).contains(&limit) {
        return Err(Error::Invalid("invalid Glyph Atlas query".into()));
    }
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = http
        .get(endpoint(base, &["layers", "characters", character])?)
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(AtlasGlyphs {
            character: character.into(),
            code_point: String::new(),
            name: None,
            readings: vec![],
            jibo: vec![],
            variants: vec![],
            variant_sources: BTreeMap::new(),
            total: 0,
            items: vec![],
            page_url: String::new(),
        });
    }
    let detail: Detail = response.error_for_status()?.json().await?;
    let mut url = endpoint(base, &["layers", "occurrences"])?;
    url.query_pairs_mut()
        .append_pair("code_point", &detail.code_point)
        .append_pair("scope", "character")
        .append_pair("limit", &limit.to_string());
    let occurrences: Occurrences = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let absolute = |path: &str| -> Result<String> {
        Ok(reqwest::Url::parse(base)
            .and_then(|b| b.join(path))
            .map_err(|e| Error::Invalid(e.to_string()))?
            .into())
    };
    let mut seen = std::collections::HashSet::new();
    let variants: Vec<AtlasVariant> = detail
        .variants
        .items
        .into_iter()
        .map(|v| AtlasVariant {
            character: v.char,
            count: v.corpus_count,
        })
        .chain(detail.variants.related.into_iter().map(|v| AtlasVariant {
            character: v.char,
            count: 0,
        }))
        .filter(|v| {
            v.character != detail.char
                && v.character.chars().count() == 1
                && seen.insert(v.character.clone())
        })
        .collect();
    let items = occurrences
        .items
        .into_iter()
        .map(|o| {
            let entry_id = o
                .document
                .as_deref()
                .and_then(|d| d.strip_prefix("hk:"))
                .filter(|e| e.len() == 32 && e.bytes().all(|b| b.is_ascii_hexdigit()))
                .map(str::to_owned);
            Ok(AtlasGlyph {
                page_index: entry_id
                    .as_ref()
                    .and(o.page_number)
                    .and_then(|n| n.checked_sub(1)),
                entry_id,
                image: absolute(&o.image)?,
                record_url: absolute(&format!("/ja/crop/{}", o.id))?,
                id: o.id,
                label: o.label,
                source: o.source,
                page_number: o.page_number,
                licence: o.licence,
                holder: o.holder,
                attribution: o.attribution,
                rights_url: o.rights_url,
                state: o.state,
            })
        })
        .collect::<Result<_>>()?;
    Ok(AtlasGlyphs {
        page_url: absolute(&format!("/ja/character/{}", detail.code_point))?,
        character: detail.char,
        code_point: detail.code_point,
        name: detail.name,
        readings: detail.readings,
        jibo: detail.jibo.into_iter().map(|j| j.char).collect(),
        variants,
        variant_sources: detail.variants.sources,
        total: occurrences.total,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path, query_param},
    };

    #[tokio::test]
    async fn detail_and_crops_link_honkoku_pages() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/layers/characters/%E5%80%99"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "char": "候", "code_point": "U+5019", "name": "CJK UNIFIED IDEOGRAPH-5019",
                "readings": [], "jibo": [],
                "variants": {"items": [{"char": "𠋫", "corpus_count": 1}, {"char": "⿰亻侯"}],
                             "related": [{"char": "𠋫"}], "sources": {"yitizi": "yitizi; MIT"}}
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/layers/occurrences"))
            .and(query_param("code_point", "U+5019"))
            .and(query_param("scope", "character"))
            .and(query_param("limit", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "total": 193,
                "items": [
                    {"id": "ar:x:8-1", "label": "候", "image": "/atlas/media/a.webp", "source": "蝦夷記行",
                     "page_number": 8, "document": "hk:0916dafb80cdc48ca7687afcad4a4f35",
                     "licence": "PD", "holder": "龍谷大学図書館", "attribution": "龍谷大学図書館",
                     "rights_url": null, "state": "pending"},
                    {"id": "ex:1:2", "label": "候", "image": "/atlas/media/b.webp", "source": "南島志",
                     "page_number": 33, "document": "hl:C61B73B39E1FDF91891AA461C2ABDE61",
                     "licence": "CC-BY-4.0", "holder": null, "attribution": null, "rights_url": null, "state": null}
                ]
            })))
            .mount(&server)
            .await;
        let result = glyphs_at(&server.uri(), "候", 2).await?;
        assert_eq!(result.total, 193);
        assert_eq!(
            result.variants,
            vec![AtlasVariant {
                character: "𠋫".into(),
                count: 1
            }]
        );
        assert_eq!(
            result.items[0].entry_id.as_deref(),
            Some("0916dafb80cdc48ca7687afcad4a4f35")
        );
        assert_eq!(result.items[0].page_index, Some(7));
        assert_eq!(
            result.items[0].image,
            format!("{}/atlas/media/a.webp", server.uri())
        );
        assert_eq!(
            result.items[0].record_url,
            format!("{}/ja/crop/ar:x:8-1", server.uri())
        );
        assert_eq!(result.items[1].entry_id, None);
        assert_eq!(result.items[1].page_index, None);
        assert_eq!(
            result.page_url,
            format!("{}/ja/character/U+5019", server.uri())
        );
        Ok(())
    }

    #[tokio::test]
    async fn unknown_character_is_empty() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(404).set_body_json(json!({"detail": "Character not found."})),
            )
            .mount(&server)
            .await;
        let result = glyphs_at(&server.uri(), "ab", 10).await?;
        assert_eq!(result.total, 0);
        assert!(result.items.is_empty());
        Ok(())
    }
}
#[cfg(test)]
mod live {
    #[tokio::test]
    #[ignore = "reads the live glyphatlas.org service"]
    async fn glyphatlas_org_answers() -> crate::Result<()> {
        let result = super::glyphs("候", 200).await?;
        assert!(result.total > 0 && !result.items.is_empty());
        assert!(
            result
                .items
                .iter()
                .filter(|g| g.entry_id.is_some())
                .all(|g| g.page_index.map(|i| i + 1) == g.page_number)
        );
        Ok(())
    }
}
