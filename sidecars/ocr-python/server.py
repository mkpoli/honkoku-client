"""Persistent, single-worker OCR service using newline-delimited JSON."""
import importlib.metadata
import json
import os
import re
import sys
import threading
from concurrent.futures import ThreadPoolExecutor

OUTPUT = sys.stdout
sys.stdout = sys.stderr
write_lock = threading.Lock()
job_lock = threading.Lock()
jobs = {}
engine = None


def emit(value):
    with write_lock:
        OUTPUT.write(json.dumps(value, ensure_ascii=False, allow_nan=False) + "\n")
        OUTPUT.flush()


def progress(request_id, stage, done, total, message):
    emit(dict(event="progress", id=request_id, stage=stage, done=done,
              total=total, message=message))


cuda_probe_result = None


def cuda_probe():
    """Creates a one-node session on the CUDA provider without CPU fallback."""
    global cuda_probe_result
    if cuda_probe_result is not None:
        return cuda_probe_result
    try:
        import onnxruntime as ort
        from onnx import TensorProto, helper
        ort.preload_dlls()
        graph = helper.make_graph(
            [helper.make_node("Identity", ["x"], ["y"])], "probe",
            [helper.make_tensor_value_info("x", TensorProto.FLOAT, [1])],
            [helper.make_tensor_value_info("y", TensorProto.FLOAT, [1])])
        model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)])
        model.ir_version = 9
        options = ort.SessionOptions()
        options.log_severity_level = 3
        options.add_session_config_entry("session.disable_cpu_ep_fallback", "1")
        session = ort.InferenceSession(model.SerializeToString(), options,
                                       providers=["CUDAExecutionProvider"])
        usable = "CUDAExecutionProvider" in session.get_providers()
        cuda_probe_result = (usable, None if usable else "CUDAExecutionProviderを作成できません。")
    except Exception as error:
        cuda_probe_result = (False, re.sub(r"\s+", " ", str(error))[:400])
    return cuda_probe_result


def status():
    from honkoku_ocr import models
    from honkoku_ocr.doctor import runtime_report
    runtime = runtime_report()
    ready = True
    try:
        models.ensure(offline=True, quiet=True, digest=True)
    except (OSError, RuntimeError):
        ready = False
    wanted = os.environ.get("HONKOKU_OCR_DEVICE") == "cuda"
    usable, cuda_error = cuda_probe() if wanted and runtime["cuda"] else (False, None)
    return dict(version=importlib.metadata.version("honkoku-ocr-py"),
                device="cuda" if wanted and usable else "cpu",
                models_ready=ready, model_version=models.DEFAULT_VERSION,
                cuda_available=bool(runtime["cuda"]) and usable,
                cuda_error=cuda_error)


def handle(request, cancelled):
    global engine
    request_id = request.get("id")
    try:
        method, params = request["method"], request.get("params") or {}
        if method == "status":
            result = status()
        elif method == "ensure_models":
            from honkoku_ocr import models
            roles = list(models.specification(models.DEFAULT_VERSION).files)
            for i, role in enumerate(roles):
                progress(request_id, "models", i, len(roles), "モデルを取得（289MB）")
                models.ensure(roles=[role], quiet=True, digest=True)
            progress(request_id, "complete", len(roles), len(roles), "完了")
            result = status()
        elif method == "process":
            from honkoku_ocr.pipeline import OCR, Box
            if engine is None:
                state = status()
                if not state["models_ready"]:
                    raise RuntimeError("モデルの準備が必要です。")
                engine = OCR(device=state["device"], quiet=True, offline=True,
                             threads=2 if state["device"] == "cuda" else 8,
                             decoder_threads=2)
            boxes = params.get("boxes")
            if boxes is not None:
                boxes = [Box(**box) for box in boxes]
            progress(request_id, "process", 0, 1, "文字を認識")
            page = engine.process(params["image_path"], boxes=boxes,
                                  layout_only=params.get("layout_only", False),
                                  cancelled=cancelled.is_set)
            result = {key: getattr(page, key) for key in
                      ("width", "height", "processed_width", "processed_height",
                       "model", "timings", "warnings")}
            result["lines"] = []
            for line in page.lines:
                value = {key: getattr(line, key) for key in
                         ("reading_order", "x", "y", "width", "height", "koji", "plain", "raw")}
                value["confidence"] = line.detection_confidence
                result["lines"].append(value)
            progress(request_id, "complete", 1, 1, "完了")
        else:
            raise ValueError("Unknown method")
        emit(dict(id=request_id, result=result))
    except Exception as error:
        message = re.sub(r"/home/[^/\s]+", "~", str(error))
        emit(dict(id=request_id, error=dict(kind=type(error).__name__,
                                           message=message or "OCRを中止しました。")))
    finally:
        with job_lock:
            jobs.pop(request_id, None)


def main():
    with ThreadPoolExecutor(max_workers=1) as worker:
        for line in sys.stdin:
            request_id = None
            try:
                request = json.loads(line)
                request_id = request.get("id")
                method = request["method"]
                if method == "cancel":
                    with job_lock:
                        flag = jobs.get(request.get("params", {}).get("id"))
                        if flag:
                            flag.set()
                    emit(dict(id=request_id, result=bool(flag)))
                elif method == "shutdown":
                    with job_lock:
                        for flag in jobs.values():
                            flag.set()
                    emit(dict(id=request_id, result=None))
                    break
                else:
                    flag = threading.Event()
                    with job_lock:
                        if request_id in jobs:
                            raise ValueError("Duplicate request id")
                        jobs[request_id] = flag
                    worker.submit(handle, request, flag)
            except Exception as error:
                emit(dict(id=request_id, error=dict(kind="protocol", message=str(error))))
        with job_lock:
            for flag in jobs.values():
                flag.set()


if __name__ == "__main__":
    main()
