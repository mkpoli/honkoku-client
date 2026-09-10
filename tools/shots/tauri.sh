#!/usr/bin/env bash
# Runs the desktop application on a virtual X display, captures its window,
# and stops it again. Needs xvfb-run, xwininfo, uv, and a built dev profile.
#
#   tools/shots/tauri.sh out.png                     home screen
#   tools/shots/tauri.sh out.png '#/entries/ID/pages/3'   any hash route
#
# GTK prefers a Wayland display when WAYLAND_DISPLAY is set (WSLg sets it),
# so the X11 backend is forced; WebKitGTK renders through llvmpipe because
# Xvfb has no GPU. The file watcher is disabled since the Tauri CLI needs an
# inotify instance for it and those are scarce on a machine running many
# development servers.
set -u
out="$1"; route="${2:-}"
repo="$(cd "$(dirname "$0")/../.." && pwd)"
display=99
log="$repo/.local/logs/tauri-shot.log"
mkdir -p "$repo/.local/logs"
cd "$repo"
unset WAYLAND_DISPLAY
export GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe MESA_LOADER_DRIVER_OVERRIDE=llvmpipe
config=()
if [ -n "$route" ]; then
  config=(--config "{\"build\":{\"devUrl\":\"http://localhost:1420/$route\"}}")
fi
xvfb-run -n "$display" -s '-screen 0 1600x1000x24' devrun bash -c "cd apps/client && exec bun x --bun --no-install tauri dev --no-watch $(printf '%q ' "${config[@]}")" > "$log" 2>&1 &
runner=$!
for _ in $(seq 1 90); do
  if DISPLAY=:$display xwininfo -root -tree 2>/dev/null | rg -q 'みんなで翻刻'; then break; fi
  if ! kill -0 "$runner" 2>/dev/null; then echo "the application ended before opening a window"; tail -8 "$log"; exit 1; fi
  sleep 2
done
sleep 20
DISPLAY=:$display uv run --quiet --with mss python -c "
import mss
with mss.MSS() as s:
    s.shot(mon=1, output='$out')
" 2>&1 | rg -v Deprecation
echo "captured $out"
for scope in $(systemctl --user list-units --type=scope --no-legend 2>/dev/null | rg -o 'devrun-[0-9]+-[0-9]+\.scope'); do
  if systemctl --user show "$scope" -p Description --value 2>/dev/null | rg -q 'apps/client|tauri dev|bun run dev$'; then systemctl --user stop "$scope"; fi
done
kill "$runner" 2>/dev/null
pkill -f "Xvfb :$display" 2>/dev/null
