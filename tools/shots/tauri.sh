#!/usr/bin/env bash
# Capture the desktop window on an isolated X display and stop its devrun scope.
set -euo pipefail
out="$1"
route="${2:-}"
repo="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo"
mkdir -p .local/logs "$(dirname "$out")"
export GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe MESA_LOADER_DRIVER_OVERRIDE=llvmpipe
unset WAYLAND_DISPLAY
export HONKOKU_SHOT_OUT="$out" HONKOKU_SHOT_ROUTE="$route"
HONKOKU_SHOT_PORT="$(bun -e 'const server = Bun.serve({port: 0, fetch: () => new Response()}); console.log(server.port); server.stop(true)')"
export HONKOKU_SHOT_PORT
xvfb-run -a -s '-screen 0 1600x1000x24' bash <<'CAPTURE'
set -euo pipefail
export TAURI_CONFIG="{\"build\":{\"devUrl\":\"http://127.0.0.1:$HONKOKU_SHOT_PORT/$HONKOKU_SHOT_ROUTE\"}}"
cargo build -p honkoku-client 2>&1 | sed -u -E 's@/home/[^/[:space:]]+@~@g'
unset TAURI_CONFIG
runner=""
cleanup() {
  if [ -n "$runner" ]; then
    while read -r scope _; do
      if [[ "$scope" == devrun-"$runner"-*.scope ]]; then
        systemctl --user kill --signal=SIGKILL "$scope" 2>/dev/null || true
        systemctl --user stop --no-block "$scope" 2>/dev/null || true
      fi
    done < <(systemctl --user list-units --plain --no-legend "devrun-$runner-*.scope")
    kill "$runner" 2>/dev/null || true
    wait "$runner" 2>/dev/null || true
    if systemctl --user list-units --plain --no-legend "devrun-$runner-*.scope" | rg -q running; then
      echo "Owned devrun scope is still running" >&2
      return 1
    fi
    echo "Owned desktop dev server stopped."
  fi
}
trap cleanup EXIT
devrun bash -c '
  set -euo pipefail
  export CHOKIDAR_USEPOLLING=1 CHOKIDAR_INTERVAL=1500
  bun run --cwd apps/client dev --port "$HONKOKU_SHOT_PORT" &
  ready=false
  for _ in $(seq 1 100); do
    if curl --silent --fail "http://127.0.0.1:$HONKOKU_SHOT_PORT/" >/dev/null; then ready=true; break; fi
    sleep 0.2
  done
  if [ "$ready" != true ]; then echo "Vite did not become ready" >&2; exit 1; fi
  exec target/debug/honkoku-client
'  > >(sed -u -E 's@/home/[^/[:space:]]+@~@g' > .local/logs/tauri-shot.log) 2>&1 &
runner=$!
ready=false
for _ in $(seq 1 90); do
  if xwininfo -root -tree 2>/dev/null | rg -q 'みんなで翻刻'; then ready=true; break; fi
  if ! kill -0 "$runner" 2>/dev/null; then tail -8 .local/logs/tauri-shot.log; exit 1; fi
  sleep 2
done
if [ "$ready" != true ]; then echo "Desktop window did not open" >&2; exit 1; fi
sleep 20
import -window root "$HONKOKU_SHOT_OUT"
test -r "$HONKOKU_SHOT_OUT"
echo "captured $HONKOKU_SHOT_OUT"
CAPTURE
