use crate::{Error, Progress, ProgressHandler, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

const SERVER: &str = include_str!("../../../sidecars/ocr-python/server.py");
#[derive(Clone)]
pub struct OcrEnvironment {
    pub directory: PathBuf,
}
#[derive(Serialize, Deserialize)]
struct Record {
    requested_gpu: bool,
    use_gpu: bool,
    versions: serde_json::Value,
}
impl OcrEnvironment {
    pub fn new(app_data_dir: impl AsRef<Path>) -> Self {
        Self {
            directory: app_data_dir.as_ref().join("ocr"),
        }
    }
    pub fn python(&self) -> PathBuf {
        self.directory.join("venv").join(if cfg!(windows) {
            "Scripts/python.exe"
        } else {
            "bin/python"
        })
    }
    pub fn ready(&self) -> bool {
        self.python().is_file() && self.directory.join("environment.json").is_file()
    }
    pub fn use_gpu(&self) -> bool {
        std::fs::read(self.directory.join("environment.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            .is_some_and(|record| record.use_gpu)
    }
    pub async fn driver_available() -> bool {
        crate::command("nvidia-smi")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success())
    }
    pub fn server(&self) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.directory)?;
        let path = self.directory.join("server.py");
        let hash = Sha256::digest(SERVER.as_bytes());
        if !std::fs::read(&path).is_ok_and(|bytes| Sha256::digest(bytes) == hash) {
            std::fs::write(&path, SERVER)?;
        }
        Ok(path)
    }
    pub async fn setup(&self, requested_gpu: bool, progress: ProgressHandler) -> Result<()> {
        if !crate::command("uv")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success())
        {
            return Err(Error::Setup(format!(
                "uvをインストールしてください: {}",
                if cfg!(windows) {
                    "powershell -ExecutionPolicy ByPass -c \"irm https://astral.sh/uv/install.ps1 | iex\""
                } else {
                    "curl -LsSf https://astral.sh/uv/install.sh | sh"
                }
            )));
        }
        std::fs::create_dir_all(&self.directory)?;
        let use_gpu = requested_gpu && Self::driver_available().await;
        let record = self.directory.join("environment.json");
        if record.exists() {
            std::fs::remove_file(&record)?;
        }
        let mut create = crate::command("uv");
        create
            .args(["venv", "--python", "3.12", "--allow-existing"])
            .arg(self.directory.join("venv"));
        stream(create, progress.clone()).await?;
        // Both wheels own the same import directory; uninstall before switching providers.
        let mut remove = crate::command("uv");
        remove
            .args(["pip", "uninstall", "--python"])
            .arg(self.python())
            .args(["onnxruntime", "onnxruntime-gpu"]);
        stream(remove, progress.clone()).await?;
        let mut install = crate::command("uv");
        install
            .args(["pip", "install", "--python"])
            .arg(self.python());
        if use_gpu {
            // onnxruntime-gpu 1.29 links CUDA 13; the runtime wheels for CUDA 13 carry no
            // "-cu13" suffix except cuDNN, and onnxruntime.preload_dlls() finds them.
            install.args([
                "honkoku-ocr-py==0.3.0",
                "onnxruntime-gpu>=1.29",
                "nvidia-cuda-runtime",
                "nvidia-cublas",
                "nvidia-cudnn-cu13",
                "nvidia-cufft",
                "nvidia-curand",
                "nvidia-cuda-nvrtc",
                "nvidia-nvjitlink",
            ]);
        } else {
            install.arg("honkoku-ocr-py[cpu]==0.3.0");
        }
        stream(install, progress).await?;
        let output = crate::command("uv")
            .args(["pip", "list", "--format", "json", "--python"])
            .arg(self.python())
            .output()
            .await?;
        if !output.status.success() {
            return Err(Error::Setup("インストールした版を取得できません。".into()));
        }
        let versions = serde_json::from_slice(&output.stdout)?;
        std::fs::write(
            record,
            serde_json::to_vec_pretty(&Record {
                requested_gpu,
                use_gpu,
                versions,
            })?,
        )?;
        self.server()?;
        Ok(())
    }
}
async fn stream(mut command: Command, progress: ProgressHandler) -> Result<()> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Setup("installer stdout unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::Setup("installer stderr unavailable".into()))?;
    let drain = |reader: Box<dyn tokio::io::AsyncRead + Unpin + Send>,
                 progress: ProgressHandler| async move {
        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await? {
            progress(Progress {
                id: 0,
                stage: "environment".into(),
                done: 0,
                total: 0,
                message: crate::redact(&line),
            });
        }
        Ok::<_, std::io::Error>(())
    };
    let (out, err) = tokio::join!(
        drain(Box::new(stdout), progress.clone()),
        drain(Box::new(stderr), progress)
    );
    out?;
    err?;
    if !child.wait().await?.success() {
        return Err(Error::Setup("OCR環境のインストールに失敗しました。".into()));
    }
    Ok(())
}
