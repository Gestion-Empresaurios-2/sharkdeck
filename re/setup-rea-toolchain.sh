#!/usr/bin/env bash
# Install the REA reverse-engineering toolchain locally (no system changes).
set -u
ROOT=/tmp/opencode/rea-toolchain
mkdir -p "$ROOT"
cd "$ROOT"

echo "== 1. Node 22 =="
if [ ! -x "$ROOT/node22/bin/node" ]; then
  curl -fsSL https://nodejs.org/dist/v22.20.0/node-v22.20.0-linux-x64.tar.xz -o node22.tar.xz \
    && tar -xf node22.tar.xz && mv node-v22.20.0-linux-x64 node22 && rm -f node22.tar.xz
fi
export PATH="$ROOT/node22/bin:$PATH"
node -v
npm -v

echo "== 2. rea-agents (REA CLI + MCP server) =="
export NPM_CONFIG_PREFIX="$ROOT/npm"
npm install -g rea-agents@latest 2>&1 | tail -5
export PATH="$ROOT/npm/bin:$PATH"
rea --version 2>&1 | head -3
rea --help 2>&1 | head -40

echo "== 3. firmware tools =="
if command -v uv >/dev/null 2>&1; then
  uv tool install unblob 2>&1 | tail -3
  uv tool install binwalk 2>&1 | tail -3
fi
ls -la "$HOME/.local/bin" 2>/dev/null | grep -Ei "unblob|binwalk" || true

echo "== 4. ghidra (native analysis) =="
if [ ! -d "$ROOT/ghidra" ]; then
  curl -fsSL https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_11.3.2_build/ghidra_11.3.2_PUBLIC_20250415.zip -o ghidra.zip 2>/dev/null \
    && unzip -q ghidra.zip && mv ghidra_11.3.2_PUBLIC ghidra && rm -f ghidra.zip \
    || echo "ghidra download skipped/failed"
fi
ls -d "$ROOT/ghidra" 2>/dev/null || echo "no ghidra dir"
echo "SETUP-REA-DONE"
