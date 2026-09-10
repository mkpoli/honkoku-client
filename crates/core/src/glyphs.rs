//! Cached corpus pages and source-image metadata for glyph comparison.
use crate::{Error, HonkokuClient, Result, cache::blocking, model::Canvas};
use futures_util::{StreamExt, stream};
use honkoku_search::Results;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlyphOccurrence {
    pub column: usize,
    pub plain: String,
    pub offset: usize,
    pub before: String,
    pub matched: String,
    pub after: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attestation {
    pub page_id: String,
    pub entry_id: String,
    pub project_id: String,
    pub index: u64,
    pub entry_label: String,
    pub project_title: String,
    pub canvas: Option<Canvas>,
    pub text: String,
    pub ocr: Value,
    pub occurrences: Vec<GlyphOccurrence>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attestations {
    pub total: u64,
    pub facets: Vec<(String, u64)>,
    pub pages: Vec<Attestation>,
    pub firestore_reads: usize,
}
impl HonkokuClient {
    pub(crate) async fn glyph_entry(&self, id: &str) -> Result<Value> {
        let key = id.to_owned();
        if let Some(entry) =
            blocking(&self.home_storage, move |db| db.get_entry::<Value>(&key)).await?
        {
            return Ok(entry);
        }
        let value: Value = self.document(&format!("entries/{id}")).await?;
        let copy = value.clone();
        blocking(&self.home_storage, move |db| db.put_entry(&copy)).await?;
        Ok(value)
    }

    /// Enrich at most 200 hit pages, with at most two transcription batch reads.
    pub async fn glyph_attestations(&self, character: &str, hits: Results) -> Result<Attestations> {
        if hits.hits.len() > 200 || character.is_empty() {
            return Err(Error::Invalid(
                "glyph query requires text and at most 200 pages".into(),
            ));
        }
        let ids: Vec<_> = hits
            .hits
            .iter()
            .map(|h| format!("{}_{}", h.entry_id, h.index))
            .collect();
        let lookup = ids.clone();
        let mut pages: HashMap<String, Value> = blocking(&self.home_storage, move |db| {
            let mut pages = HashMap::new();
            for id in lookup {
                if let Some(page) = db.get_page::<Value>(&id)? {
                    pages.insert(id, page);
                }
            }
            Ok(pages)
        })
        .await?;
        let missing: Vec<_> = ids
            .iter()
            .filter(|id| !pages.contains_key(*id))
            .cloned()
            .collect();
        let mut failures = HashMap::new();
        for batch in missing.chunks(100) {
            match self
                .batch_get(batch.iter().map(|id| format!("transcriptions/{id}")))
                .await
            {
                Ok(documents) => {
                    let mut fetched = Vec::new();
                    for (id, document) in batch.iter().zip(documents) {
                        if let Some(document) = document {
                            let mut page = crate::firestore::plain_value(&document.fields);
                            page["id"] = serde_json::json!(id);
                            page["_firestore"] = serde_json::json!({"name":document.name,"updateTime":document.update_time});
                            if page["entryId"]
                                .as_str()
                                .zip(page["index"].as_u64())
                                .is_none_or(|(entry, index)| format!("{entry}_{index}") != *id)
                            {
                                return Err(Error::Invalid("glyph page identity mismatch".into()));
                            }
                            fetched.push(page.clone());
                            pages.insert(id.clone(), page);
                        } else {
                            failures.insert(id.clone(), "翻刻が見つかりません。".to_owned());
                        }
                    }
                    blocking(&self.home_storage, move |db| {
                        db.transaction(|db| {
                            for page in fetched {
                                db.put_page(&page)?;
                            }
                            Ok(())
                        })
                    })
                    .await?;
                }
                Err(_) => {
                    for id in batch {
                        failures.insert(
                            id.clone(),
                            "翻刻を取得できません。再試行してください。".to_owned(),
                        );
                    }
                }
            }
        }
        let mut entries: Vec<_> = hits.hits.iter().map(|h| h.entry_id.clone()).collect();
        entries.sort();
        entries.dedup();
        let entries: HashMap<_, _> = stream::iter(entries)
            .map(|id| async move {
                let entry = self.glyph_entry(&id).await.ok();
                (id, entry)
            })
            .buffer_unordered(4)
            .collect()
            .await;
        let mut project_ids: Vec<_> = hits
            .hits
            .iter()
            .filter(|h| h.project_title.is_empty())
            .map(|h| h.project_id.clone())
            .collect();
        project_ids.sort();
        project_ids.dedup();
        let project_titles: HashMap<_, _> = stream::iter(project_ids)
            .map(|id| async move {
                let title = self
                    .cached_project(&self.home_storage, &id, false)
                    .await
                    .ok()
                    .map(|p| p.title)
                    .unwrap_or_default();
                (id, title)
            })
            .buffer_unordered(4)
            .collect()
            .await;
        let mut out = Vec::new();
        for (hit, id) in hits.hits.into_iter().zip(ids) {
            let page = pages.get(&id);
            let entry = entries.get(&hit.entry_id).and_then(Option::as_ref);
            let canvas = entry.and_then(|e| canvas(e, hit.index as usize));
            let text = page
                .and_then(|p| p["text"].as_str())
                .unwrap_or_default()
                .to_owned();
            let occurrences = if page.is_some() {
                occurrences(&text, character)
            } else {
                hit.occurrences
                    .into_iter()
                    .map(|o| GlyphOccurrence {
                        column: o.column,
                        plain: format!("{}{}{}", o.before, o.matched, o.after),
                        offset: o.before.chars().count(),
                        before: o.before,
                        matched: o.matched,
                        after: o.after,
                    })
                    .collect()
            };
            out.push(Attestation {
                page_id: id.clone(),
                entry_id: hit.entry_id,
                project_id: hit.project_id.clone(),
                index: hit.index,
                entry_label: hit.entry_label,
                project_title: if hit.project_title.is_empty() {
                    project_titles
                        .get(&hit.project_id)
                        .cloned()
                        .unwrap_or_default()
                } else {
                    hit.project_title
                },
                canvas,
                text,
                ocr: page.map(|p| p["ocr"].clone()).unwrap_or(Value::Null),
                occurrences,
                error: failures.remove(&id).or_else(|| {
                    entry
                        .is_none()
                        .then(|| "原本の情報を取得できません。".into())
                }),
            });
        }
        Ok(Attestations {
            total: hits.total,
            facets: hits.facets,
            pages: out,
            firestore_reads: missing.len(),
        })
    }
}
pub(crate) fn canvas(entry: &Value, index: usize) -> Option<Canvas> {
    let value = entry.get("canvases")?.get(index)?;
    let resource = &value["images"][0]["resource"];
    let service = if resource["service"].is_array() {
        &resource["service"][0]
    } else {
        &resource["service"]
    };
    let info = value["infoJsonUrl"]
        .as_str()
        .or_else(|| entry["tileSources"][index].as_str())
        .map(str::to_owned)
        .or_else(|| {
            service["@id"]
                .as_str()
                .or_else(|| service["id"].as_str())
                .map(|id| format!("{}/info.json", id.trim_end_matches('/')))
        });
    Some(Canvas {
        id: value["id"]
            .as_str()
            .or_else(|| value["@id"].as_str())
            .unwrap_or_default()
            .into(),
        width: u32::try_from(value["width"].as_u64()?).ok()?,
        height: u32::try_from(value["height"].as_u64()?).ok()?,
        info_json_url: info,
        image_url: value["imageUrl"]
            .as_str()
            .or_else(|| resource["@id"].as_str())
            .or_else(|| resource["id"].as_str())
            .map(str::to_owned),
        thumbnail_url: None,
        extra: Default::default(),
    })
}
fn occurrences(source: &str, character: &str) -> Vec<GlyphOccurrence> {
    let mut result = Vec::new();
    for (column, line) in source
        .split(['\n', '\r'])
        .filter(|l| !l.trim().is_empty() && !matches!(l.trim(), "【右丁】" | "【左丁】"))
        .enumerate()
    {
        let plain = honkoku_text::plain_text(line);
        let chars: Vec<_> = plain.chars().collect();
        for (offset, (byte, _)) in plain.char_indices().enumerate() {
            if !plain[byte..].starts_with(character) {
                continue;
            }
            let end = offset + character.chars().count();
            result.push(GlyphOccurrence {
                column,
                plain: plain.clone(),
                offset,
                before: chars[offset.saturating_sub(20)..offset].iter().collect(),
                matched: character.into(),
                after: chars[end..(end + 20).min(chars.len())].iter().collect(),
            });
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_count_scalars_and_skip_dividers() {
        let rows = occurrences("【右丁】\r\n\n𛀁《振り仮名：候｜そうろう》候\n文候", "候");
        assert_eq!(
            rows.iter()
                .map(|r| (r.column, r.offset))
                .collect::<Vec<_>>(),
            vec![(0, 1), (0, 2), (1, 1)]
        );
        assert_eq!(rows[0].plain, "𛀁候候");
    }
    #[test]
    fn transcription_ids_match_capture() -> Result<()> {
        let value = serde_json::from_str(include_str!(
            "../../../fixtures/api/firestore-pages-0916dafb.json"
        ))?;
        for page in crate::firestore::decode_pages(&value)? {
            assert_eq!(page.id, format!("{}_{}", page.entry_id, page.index));
        }
        Ok(())
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::firestore::encode_value;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };
    fn hits() -> Results {
        Results {
            total: 200,
            facets: vec![],
            next_cursor: None,
            hits: (0..200)
                .map(|index| honkoku_search::Hit {
                    page_id: format!("p/e/{index}"),
                    entry_id: "e".into(),
                    project_id: "p".into(),
                    index,
                    entry_label: "資料".into(),
                    project_title: "事業".into(),
                    occurrences: vec![],
                })
                .collect(),
        }
    }
    #[tokio::test]
    async fn batches_are_bounded_and_second_read_is_cached() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/documents:batchGet")).respond_with(|request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let documents = body["documents"].as_array().unwrap();
            assert_eq!(documents.len(),100);
            let rows:Vec<_> = documents.iter().map(|name| {
                let index = name.as_str().unwrap().rsplit('_').next().unwrap().parse::<u32>().unwrap();
                json!({"found":{"name":name,"updateTime":"2026-09-10T00:00:00Z","fields":encode_value(&json!({"entryId":"e","index":index,"text":"御座候","status":"completed","notes":[],"ocr":[]}))["mapValue"]["fields"]}})
            }).collect();
            ResponseTemplate::new(200).set_body_json(rows)
        }).expect(2).mount(&server).await;
        let client =
            HonkokuClient::with_endpoints(&server.uri(), &format!("{}/documents", server.uri()))?;
        blocking(&client.home_storage, |db| {
            db.put_entry(&json!({"id":"e","collectionId":"c","projectId":"p","index":0}))
        })
        .await?;
        let first = client.glyph_attestations("候", hits()).await?;
        assert_eq!(first.firestore_reads, 200);
        assert_eq!(first.pages.len(), 200);
        assert!(first.pages.iter().all(|p| p.occurrences.len() == 1));
        let warm = client.glyph_attestations("候", hits()).await?;
        assert_eq!(warm.firestore_reads, 0);
        Ok(())
    }
}
