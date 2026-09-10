#![cfg(unix)]
use honkoku_ocr::{OcrEngine, OcrEnvironment, OcrSidecar};
use serde_json::json;
use std::sync::Arc;

const FAKE: &str = r#"
import json,sys,os,threading,time
flag=threading.Event()
lock=threading.Lock()
def emit(v):
 with lock:
  print(json.dumps(v),flush=True)
def process(r):
 flag.wait(5)
 emit({'id':r['id'],'error':{'kind':'ProcessingCancelled','message':'cancelled'}})
for raw in sys.stdin:
 r=json.loads(raw);m=r['method'];i=r['id']
 if m=='shutdown': break
 if m=='crash': os._exit(7)
 if m=='stderr': print('sidecar diagnostic line',file=sys.stderr,flush=True)
 if m=='cancel': flag.set();emit({'id':i,'result':True})
 elif m=='process':
  emit({'id':i,'event':'progress','stage':'process','done':0,'total':1,'message':'ready'})
  threading.Thread(target=process,args=(r,)).start()
 else:
  emit({'id':i,'event':'progress','stage':'test','done':1,'total':1,'message':'ready'})
  emit({'id':i,'result':{'pid':os.getpid(),'echo':r['params']}})
"#;
fn worker() -> Result<(tempfile::TempDir, Arc<OcrSidecar>), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let environment = OcrEnvironment::new(temp.path());
    std::fs::create_dir_all(environment.python().parent().ok_or("missing parent")?)?;
    std::os::unix::fs::symlink("/usr/bin/python3", environment.python())?;
    let script = temp.path().join("fake.py");
    std::fs::write(&script, FAKE)?;
    Ok((temp, Arc::new(OcrSidecar::with_script(environment, script))))
}
#[tokio::test]
async fn routes_progress_serializes_requests_and_recovers_crash()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp, worker) = worker()?;
    let events = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let capture = events.clone();
    let progress: honkoku_ocr::ProgressHandler = Arc::new(move |_| {
        capture.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    let (first, second) = tokio::join!(
        worker.request("echo", json!(1), progress.clone()),
        worker.request("echo", json!(2), progress.clone())
    );
    let first = first?;
    let second = second?;
    assert_eq!(first["pid"], second["pid"]);
    assert_eq!(first["echo"], 1);
    assert_eq!(second["echo"], 2);
    assert_eq!(events.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(
        worker
            .request("crash", json!({}), progress.clone())
            .await
            .is_err()
    );
    let restarted = worker.request("echo", json!(3), progress).await?;
    assert_ne!(first["pid"], restarted["pid"]);
    worker.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn cancellation_bypasses_the_request_queue() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp, worker) = worker()?;
    let ready = Arc::new(tokio::sync::Notify::new());
    let notify = ready.clone();
    let task_worker = worker.clone();
    let task = tokio::spawn(async move {
        task_worker
            .request("process", json!({}), Arc::new(move |_| notify.notify_one()))
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), ready.notified()).await?;
    worker.cancel().await?;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await??
            .is_err()
    );
    worker.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn stderr_is_written_to_the_configured_log() -> Result<(), Box<dyn std::error::Error>> {
    let (temp, worker) = worker()?;
    worker
        .request("stderr", json!({}), Arc::new(|_| {}))
        .await?;
    worker.shutdown().await?;
    let path = temp.path().join("ocr/ocr.log");
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if std::fs::read_to_string(&path).is_ok_and(|s| s.contains("sidecar diagnostic line")) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    Ok(())
}
#[tokio::test]
async fn missing_environment_has_actionable_diagnostics() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let worker = OcrSidecar::new(OcrEnvironment::new(temp.path()))
        .with_log_path(temp.path().join("logs/ocr.log"));
    let diagnostics = worker.diagnostics().await?;
    assert!(!diagnostics.environment_ready);
    assert!(!diagnostics.models_present);
    assert_eq!(diagnostics.models_bytes, 0);
    assert!(diagnostics.log_path.ends_with("logs/ocr.log"));
    assert!(
        !diagnostics
            .status
            .ok_or("missing status")?
            .environment_ready
    );
    Ok(())
}
