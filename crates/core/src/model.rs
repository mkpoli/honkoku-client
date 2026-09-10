//! Public read models. Unknown fields survive transport and cache round trips.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Nanosecond-precision instant; serialized to RFC 3339 for the IPC boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timestamp(pub OffsetDateTime);
impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.format(&Rfc3339).map_err(serde::ser::Error::custom)?)
    }
}
impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Text(String),
            Express { _seconds: i64, _nanoseconds: u32 },
        }
        let instant = match Wire::deserialize(deserializer)? {
            Wire::Text(text) => {
                OffsetDateTime::parse(&text, &Rfc3339).map_err(serde::de::Error::custom)?
            }
            Wire::Express {
                _seconds,
                _nanoseconds,
            } => {
                if _nanoseconds >= 1_000_000_000 {
                    return Err(serde::de::Error::custom("invalid nanoseconds"));
                }
                OffsetDateTime::from_unix_timestamp_nanos(
                    i128::from(_seconds) * 1_000_000_000 + i128::from(_nanoseconds),
                )
                .map_err(serde::de::Error::custom)?
            }
        };
        Ok(Self(instant))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageStatus {
    Default,
    Initiated,
    Editing,
    Completed,
    Frozen,
    Unknown(String),
}
impl PageStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Default => "default",
            Self::Initiated => "initiated",
            Self::Editing => "editing",
            Self::Completed => "completed",
            Self::Frozen => "frozen",
            Self::Unknown(value) => value,
        }
    }
}
impl Serialize for PageStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}
impl<'de> Deserialize<'de> for PageStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer)?.as_str() {
            "default" => Self::Default,
            "initiated" => Self::Initiated,
            "editing" => Self::Editing,
            "completed" => Self::Completed,
            "frozen" => Self::Frozen,
            other => Self::Unknown(other.into()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Label {
    Text(String),
    Languages(BTreeMap<String, Vec<String>>),
}
impl Label {
    pub fn preferred(&self, languages: &[&str]) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Languages(map) => languages
                .iter()
                .filter_map(|lang| map.get(*lang))
                .find(|v| !v.is_empty())
                .or_else(|| map.values().find(|v| !v.is_empty()))
                .map(|v| v.join(" / "))
                .unwrap_or_default(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Keywords {
    Text(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Keywords>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_users: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_own_guidelines: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidelines: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functions: Option<ProjectFunctions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_entry_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_entry_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_image_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_image_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collections: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFunctions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "enableOCR")]
    pub enable_ocr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_translations: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub project_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub project_id: String,
    pub collection_id: String,
    pub index: u32,
    pub label: Label,
    pub manifest_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvases: Option<Vec<Canvas>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcriptions: Option<Vec<Page>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Canvas {
    pub id: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_json_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    pub entry_id: String,
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_id: Option<String>,
    pub status: PageStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_status: Option<PageStatus>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp_text_changed: Option<bool>,
    #[serde(default)]
    pub notes: Vec<Option<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp_notes: Option<Vec<Option<Value>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translations: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<Ocr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_review: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp_edited_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Ocr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ndl: Option<OcrResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minna: Option<OcrResult>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub uid: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub like_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stone_count: Option<u64>,
    #[serde(default, rename = "photoURL", skip_serializing_if = "Option::is_none")]
    pub photo_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub uid: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
    pub state: String,
    pub created_at: Timestamp,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub display: bool,
    pub created_at: Timestamp,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub id: String,
    pub uid: String,
    pub project_id: String,
    pub entry_id: String,
    pub transcription_id: String,
    pub index: u32,
    pub event_type: String,
    pub count: i64,
    pub is_review: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_review: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_approval: Option<bool>,
    pub created_at: Timestamp,
    pub data: Value,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    pub event: TimelineEvent,
    // Deleted enrichment documents do not remove the activity itself.
    pub actor: Option<User>,
    pub entry_label: Option<Label>,
    pub project_title: Option<String>,
    pub excerpt: String,
}

/// Observed via public runQuery on 2026-09-10: date IDs (YYYY-MM-DD), timestamp,
/// integer site counters below, and projects keyed by project ID. Each project
/// has five counters and participants: { uid: boolean }. The kirishitan project's
/// completedImageCount is Firestore doubleValue "NaN", retained as unavailable
/// rather than inventing a count. No per-project
/// initiated/default image counters were present in the two latest documents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyProgress {
    pub id: String,
    pub timestamp: Timestamp,
    pub total_entry_count: u64,
    pub completed_entry_count: u64,
    pub total_image_count: u64,
    pub completed_image_count: u64,
    pub initiated_image_count: u64,
    pub default_image_count: u64,
    pub char_count: u64,
    pub user_count: u64,
    pub projects: BTreeMap<String, ProjectProgress>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProgress {
    pub total_entry_count: u64,
    pub completed_entry_count: u64,
    pub total_image_count: u64,
    pub completed_image_count: ProgressCount,
    pub char_count: u64,
    #[serde(default)]
    pub participants: BTreeMap<String, bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Firestore permits non-finite doubles, serialized as strings in its REST API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ProgressCount {
    Integer(u64),
    Double(f64),
    NonFinite(NonFiniteNumber),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NonFiniteNumber {
    #[serde(rename = "NaN")]
    Nan,
    Infinity,
    #[serde(rename = "-Infinity")]
    NegativeInfinity,
}

/// Both older plain strings and current structured OCR results occur upstream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum OcrResult {
    Text(String),
    Structured(OcrRecord),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecord {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_blocks: Option<Vec<Value>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    pub id: String,
    pub project_id: String,
    pub collection_id: String,
    pub index: u32,
    pub label: Label,
    pub manifest_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatusCounts {
    pub completed: u64,
    pub initiated: u64,
    pub editing: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionProgress {
    pub collection_id: String,
    pub entries: usize,
    pub size: u64,
    #[serde(flatten)]
    pub counts: StatusCounts,
    pub fetched_at: Timestamp,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntryProgress {
    pub entry_id: String,
    pub size: u64,
    #[serde(flatten)]
    pub counts: StatusCounts,
    pub fetched_at: Timestamp,
}
