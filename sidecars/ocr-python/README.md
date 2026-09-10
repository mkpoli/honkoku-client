# OCR sidecar

Python3.12の常駐プロセス。標準入力と標準出力にUTF-8の改行区切りJSONを使用する。診断は標準エラー出力に送る。同時に実行する認識処理は1件。

要求は`{"id":1,"method":"status","params":{}}`。応答は`{"id":1,"result":…}`または`{"id":1,"error":{"kind":"…","message":"…"}}`。

| method | params | result |
| --- | --- | --- |
| `status` | `{}` | `version, device, models_ready, model_version, cuda_available` |
| `ensure_models` | `{}` | SHA-256照合後の`status` |
| `process` | `image_path, boxes?: [{x,y,width,height}], layout_only?: bool` | `width, height, processed_width, processed_height, model, lines, timings, warnings` |
| `cancel` | `id`（対象の要求ID） | 対象の有無。実行中の推論が終わった境界で中止する |
| `shutdown` | `{}` | `null`。処理を中止して終了する |

進捗通知は`{"event":"progress","id":1,"stage":"models","done":1,"total":4,"message":"…"}`。`stage`は`environment, models, process, complete`。

行は読み順に並び、`reading_order, x, y, width, height, confidence, koji, plain, raw`を含む。座標は入力画像のEXIF回転適用後の画素空間。`confidence`は行検出の確信度。`raw`はモデルのHTML形式、`koji`は挿入用記法。`timings`の単位は秒。

環境管理側がCPU版またはGPU版を選んで`honkoku-ocr-py==0.3.0`をインストールする。GPU環境は`HONKOKU_OCR_DEVICE=cuda`を指定する。モデルの保存先は`HONKOKU_OCR_MODELS`。モデルの取得には約289MB、GPUランタイムには追加のディスク容量が必要。
