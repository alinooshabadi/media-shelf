#!/usr/bin/env bash
# Serves the live component site on http://127.0.0.1:${PORT:-8765} (local only).
HERE="$(cd "$(dirname "$0")/.." && pwd)"
PORT=${PORT:-8765}
[[ -f "$HERE/site/wasm/scenes.wasm" ]] || "$HERE/tools/build.sh"
echo "CUI live components: http://localhost:$PORT"
exec python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$HERE/site"
