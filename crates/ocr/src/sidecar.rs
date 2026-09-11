use crate::{
    Error, OcrBox, OcrEngine, OcrEnvironment, OcrPage, OcrStatus, ProgressHandler, Result,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
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
    log_path: PathBuf,
    last_error: Mutex<Option<String>>,
}
impl OcrSidecar {
    pub fn new(environment: OcrEnvironment) -> Self {
        Self {
            log_path: environment.directory.join("ocr.log"),
            last_error: Mutex::new(None),
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
    pub fn with_log_path(mut self, path: PathBuf) -> Self {
        self.log_path = path;
        self
    }
    fn log_file(&self) -> Result<std::fs::File> {
        if let Some(parent) = self.log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?)
    }
    async fn diagnostic_command(&self, action: &str) -> Result<String> {
        let output = crate::command(self.environment.python())
            .args(["-c", include_str!("diagnostics.py"), action])
            .env(
                "HONKOKU_OCR_MODELS",
                self.environment.directory.join("models"),
            )
            .env(
                "HONKOKU_OCR_DEVICE",
                if self.environment.use_gpu() {
                    "cuda"
                } else {
                    "cpu"
                },
            )
            .kill_on_drop(true)
            .output()
            .await?;
        let stderr = crate::redact(&String::from_utf8_lossy(&output.stderr));
        self.log_file()?.write_all(stderr.as_bytes())?;
        let text = crate::redact(&String::from_utf8_lossy(&output.stdout));
        if !output.status.success() {
            let message = if text.trim().is_empty() { stderr } else { text };
            *self.last_error.lock().await = Some(message.clone());
            return Err(Error::Setup(message));
        }
        Ok(text)
    }
    pub async fn doctor(&self) -> Result<String> {
        self.diagnostic_command("doctor").await
    }
    pub async fn repair_models(&self, progress: ProgressHandler) -> Result<OcrStatus> {
        let _gate = self.gate.lock().await;
        self.stop().await?;
        self.diagnostic_command("repair").await?;
        let mut status: OcrStatus =
            serde_json::from_value(self.request_locked("status", json!({}), progress).await?)?;
        status.environment_ready = true;
        Ok(status)
    }
    pub async fn diagnostics(&self) -> Result<crate::OcrDiagnostics> {
        let status = match self.status().await {
            Ok(status) => Some(status),
            Err(error) => {
                *self.last_error.lock().await = Some(crate::redact(&error.to_string()));
                None
            }
        };
        if self.environment.ready() && status.as_ref().is_some_and(|s| !s.models_ready) {
            let _ = self.diagnostic_command("verify").await;
        }
        let models = self.environment.directory.join("models");
        let mut bytes = 0;
        if models.is_dir() {
            for entry in std::fs::read_dir(&models)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    bytes += entry.metadata()?.len();
                }
            }
        }
        Ok(crate::OcrDiagnostics {
            status,
            environment_ready: self.environment.ready(),
            models_present: bytes > 0,
            models_directory_exists: models.is_dir(),
            models_bytes: bytes,
            last_error: self.last_error.lock().await.clone(),
            log_path: crate::redact(&self.log_path.to_string_lossy()),
        })
    }
    async fn spawn(&self) -> Result<Process> {
        let script = match &self.script {
            Some(script) => script.clone(),
            None => self.environment.server()?,
        };
        let mut child = crate::command(self.environment.python())
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
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Setup("sidecar stderr unavailable".into()))?;
        let mut log = self.log_file()?;
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if writeln!(log, "{}", crate::redact(&line)).is_err() {
                    break;
                }
            }
        });
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
                    *self.last_error.lock().await = Some(crate::redact(
                        error["message"].as_str().unwrap_or("OCR failed"),
                    ));
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
                cuda_error: None,
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
