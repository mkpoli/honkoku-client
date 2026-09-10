use crate::{
    Error, OcrBox, OcrEngine, OcrEnvironment, OcrPage, OcrStatus, ProgressHandler, Result,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};
type Input = Arc<Mutex<ChildStdin>>;
struct Process {
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
}
pub struct OcrSidecar {
    pub environment: OcrEnvironment,
    gate: Mutex<()>,
    process: Mutex<Option<Process>>,
    input: Mutex<Option<Input>>,
    sequence: AtomicU64,
    active: AtomicU64,
    script: Option<PathBuf>,
}
impl OcrSidecar {
    pub fn new(environment: OcrEnvironment) -> Self {
        Self {
            environment,
            gate: Mutex::new(()),
            process: Mutex::new(None),
            input: Mutex::new(None),
            sequence: AtomicU64::new(1),
            active: AtomicU64::new(0),
            script: None,
        }
    }
    pub fn with_script(environment: OcrEnvironment, script: PathBuf) -> Self {
        Self {
            script: Some(script),
            ..Self::new(environment)
        }
    }
    async fn spawn(&self) -> Result<Process> {
        let script = match &self.script {
            Some(script) => script.clone(),
            None => self.environment.server()?,
        };
        let mut child = Command::new(self.environment.python())
            .arg("-u")
            .arg(script)
            .env(
                "HONKOKU_OCR_DEVICE",
                if self.environment.use_gpu() {
                    "cuda"
                } else {
                    "cpu"
                },
            )
            .env(
                "HONKOKU_OCR_MODELS",
                self.environment.directory.join("models"),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| Error::Setup("sidecar stdin unavailable".into()))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| Error::Setup("sidecar stdout unavailable".into()))?;
        *self.input.lock().await = Some(Arc::new(Mutex::new(input)));
        Ok(Process {
            child,
            lines: BufReader::new(output).lines(),
        })
    }
    async fn send(&self, value: Value) -> Result<()> {
        let input = self
            .input
            .lock()
            .await
            .clone()
            .ok_or_else(|| Error::Setup("OCR worker is not running".into()))?;
        let mut input = input.lock().await;
        let mut bytes = serde_json::to_vec(&value)?;
        bytes.push(b'\n');
        input.write_all(&bytes).await?;
        input.flush().await?;
        Ok(())
    }
    pub async fn request(
        &self,
        method: &str,
        params: Value,
        progress: ProgressHandler,
    ) -> Result<Value> {
        let _gate = self.gate.lock().await;
        self.request_locked(method, params, progress).await
    }
    async fn request_locked(
        &self,
        method: &str,
        params: Value,
        progress: ProgressHandler,
    ) -> Result<Value> {
        let mut slot = self.process.lock().await;
        if slot.is_none() {
            *slot = Some(self.spawn().await?);
        }
        let id = self.sequence.fetch_add(1, Ordering::SeqCst);
        if method == "process" {
            self.active.store(id, Ordering::SeqCst);
        }
        let result = async {
            self.send(json!({"id":id,"method":method,"params":params}))
                .await?;
            let process = slot
                .as_mut()
                .ok_or_else(|| Error::Setup("worker unavailable".into()))?;
            loop {
                let line = process
                    .lines
                    .next_line()
                    .await?
                    .ok_or_else(|| Error::Worker {
                        kind: "crash".into(),
                        message: "OCRプロセスが終了しました。再実行すると再起動します。".into(),
                    })?;
                let value: Value = serde_json::from_str(&line)?;
                if value["id"] != id {
                    continue;
                }
                if value["event"] == "progress" {
                    progress(serde_json::from_value(value)?);
                    continue;
                }
                if let Some(error) = value.get("error") {
                    return Err(Error::Worker {
                        kind: error["kind"].as_str().unwrap_or("ocr").into(),
                        message: crate::redact(error["message"].as_str().unwrap_or("OCR failed")),
                    });
                }
                return value
                    .get("result")
                    .cloned()
                    .ok_or_else(|| Error::Setup("invalid sidecar response".into()));
            }
        }
        .await;
        self.active.store(0, Ordering::SeqCst);
        if result
            .as_ref()
            .is_err_and(|error| !matches!(error, Error::Worker { kind, .. } if kind != "crash"))
        {
            if let Some(mut process) = slot.take() {
                let _ = process.child.kill().await;
            }
            *self.input.lock().await = None;
        }
        result
    }
    pub async fn setup(&self, use_gpu: bool, progress: ProgressHandler) -> Result<OcrStatus> {
        let _gate = self.gate.lock().await;
        self.stop().await?;
        self.environment.setup(use_gpu, progress.clone()).await?;
        let value = self
            .request_locked("ensure_models", json!({}), progress)
            .await?;
        let mut status: OcrStatus = serde_json::from_value(value)?;
        status.environment_ready = true;
        Ok(status)
    }
    async fn stop(&self) -> Result<()> {
        let mut slot = self.process.lock().await;
        if let Some(mut process) = slot.take() {
            let _ = self
                .send(json!({"id":0,"method":"shutdown","params":{}}))
                .await;
            if tokio::time::timeout(std::time::Duration::from_secs(5), process.child.wait())
                .await
                .is_err()
            {
                process.child.kill().await?;
            }
        }
        *self.input.lock().await = None;
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.cancel().await?;
        let _gate = self.gate.lock().await;
        self.stop().await
    }
}
impl OcrEngine for OcrSidecar {
    async fn status(&self) -> Result<OcrStatus> {
        if !self.environment.ready() {
            return Ok(OcrStatus {
                version: None,
                device: "cpu".into(),
                models_ready: false,
                model_version: "v18".into(),
                cuda_available: OcrEnvironment::driver_available().await,
                environment_ready: false,
            });
        }
        let mut status: OcrStatus =
            serde_json::from_value(self.request("status", json!({}), Arc::new(|_| {})).await?)?;
        status.environment_ready = true;
        Ok(status)
    }
    async fn ensure_models(&self, progress: ProgressHandler) -> Result<OcrStatus> {
        let mut status: OcrStatus =
            serde_json::from_value(self.request("ensure_models", json!({}), progress).await?)?;
        status.environment_ready = true;
        Ok(status)
    }
    async fn process_page(
        &self,
        image_path: &Path,
        boxes: Option<Vec<OcrBox>>,
        progress: ProgressHandler,
    ) -> Result<OcrPage> {
        Ok(serde_json::from_value(
            self.request(
                "process",
                json!({"image_path":image_path,"boxes":boxes}),
                progress,
            )
            .await?,
        )?)
    }
    async fn cancel(&self) -> Result<()> {
        let id = self.active.load(Ordering::SeqCst);
        if id != 0 {
            self.send(json!({"id":0,"method":"cancel","params":{"id":id}}))
                .await?;
        }
        Ok(())
    }
}
