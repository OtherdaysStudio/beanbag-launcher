#!/usr/bin/env bash
# The gate. "Done" means this exits 0.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null && command -v brew >/dev/null; then
  PATH="$(brew --prefix rustup)/bin:$PATH"
fi

echo "== types";  npx tsc --noEmit
echo "== unit";   npx vitest run
echo "== rust";   (cd src-tauri && cargo test --quiet)
echo "== build";  npx vite build --logLevel warn
echo "== smoke";  node scripts/smoke.mjs
echo "verify: ok"
