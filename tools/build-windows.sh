#!/usr/bin/env bash
# Cross-compiles the portable Windows executable from Linux with cargo-xwin
# and copies it to a destination directory (a Windows folder under /mnt/c
# when run inside WSL). Needs the x86_64-pc-windows-msvc target
# (`rustup target add x86_64-pc-windows-msvc`), cargo-xwin, and LLVM's
# clang-cl, lld-link and llvm-lib; the Windows SDK is fetched on first use.
#
#   tools/build-windows.sh [destination-dir]
set -euo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd)"
downloads() {
  # The Downloads folder may be relocated; the shell folder registry value is authoritative.
  local raw expanded
  raw=$(powershell.exe -NoProfile -Command "(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders').'{374DE290-123F-4565-9164-39C4925E467B}'" 2>/dev/null | tr -d '\r')
  [ -n "$raw" ] || return 1
  expanded=$(powershell.exe -NoProfile -Command "[Environment]::ExpandEnvironmentVariables('$raw')" 2>/dev/null | tr -d '\r')
  wslpath -u "$expanded" 2>/dev/null
}
dest="${1:-$(downloads || echo "$HOME/honkoku-client-windows")/honkoku-client}"
cd "$repo"
export PATH="/usr/lib/llvm-18/bin:$PATH" XWIN_ACCEPT_LICENSE=1
bun run --cwd apps/client tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle
mkdir -p "$dest"
cp target/x86_64-pc-windows-msvc/release/honkoku-client.exe "$dest/honkoku-client.exe"
echo "copied to $dest/honkoku-client.exe"
