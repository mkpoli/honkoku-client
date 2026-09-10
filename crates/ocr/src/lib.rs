//! Local OCR environment, persistent protocol worker, and canvas result flow.
mod environment;
mod page;
mod sidecar;
pub use environment::OcrEnvironment;
pub use page::{OcrBox, OcrLine, OcrPage, run_page};
use serde::{Deserialize, Serialize};
pub use sidecar::OcrSidecar;
use std::{path::Path, sync::Arc};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("OCR I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("OCR JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{kind}: {message}")]
    Worker { kind: String, message: String },
    #[error("{0}")]
    Setup(String),
    #[error("{0}")]
    Core(#[from] honkoku_core::Error),
    #[error("{0}")]
    Iiif(#[from] honkoku_iiif::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrStatus {
    pub version: Option<String>,
    pub device: String,
    pub models_ready: bool,
    pub model_version: String,
    pub cuda_available: bool,
    #[serde(default)]
    pub environment_ready: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub id: u64,
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub message: String,
}
pub type ProgressHandler = Arc<dyn Fn(Progress) + Send + Sync>;
pub trait OcrEngine: Send + Sync {
    fn status(&self) -> impl Future<Output = Result<OcrStatus>> + Send;
    fn ensure_models(
        &self,
        progress: ProgressHandler,
    ) -> impl Future<Output = Result<OcrStatus>> + Send;
    fn process_page(
        &self,
        image_path: &Path,
        boxes: Option<Vec<OcrBox>>,
        progress: ProgressHandler,
    ) -> impl Future<Output = Result<OcrPage>> + Send;
    fn cancel(&self) -> impl Future<Output = Result<()>> + Send;
}
pub fn redact(message: &str) -> String {
    let mut output = message.to_owned();
    while let Some(start) = output.find("/home/") {
        let end = output[start + 6..]
            .find(['/', ' ', '\n', '\r', '\'', '"'])
            .map_or(output.len(), |n| start + 6 + n);
        output.replace_range(start..end, "~");
    }
    output
}
