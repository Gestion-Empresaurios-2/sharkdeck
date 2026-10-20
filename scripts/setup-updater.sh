#!/usr/bin/env bash
# One-time updater setup for SharkDeck.
#
# It does the sensitive part ON YOUR MACHINE (a private signing key is never
# handed to an agent or committed): generates a Tauri updater keypair, writes the
# public key into tauri.conf.json, and stores the private key in .updater/ for
# the release build.
#
#   ./scripts/setup-updater.sh
#
# Afterwards: add the printed TAURI_SIGNING_PRIVATE_KEY / _PASSWORD as GitHub
# repository secrets so the release workflow can sign update artifacts.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

KEY_DIR="$ROOT/.updater"
KEY="$KEY_DIR/signing.key"
mkdir -p "$KEY_DIR"

if [ -f "$KEY" ]; then
  echo "→ key already exists at $KEY (delete it to rotate)"
else
  echo "→ generating updater keypair (choose a password — you will need it in CI)"
  npx tauri signer generate --writable-keys -w "$KEY"
fi

PUB_B64="$(python3 - <<'PY'
import base64, pathlib, re, sys
# The CLI prints the base64 public key; the .pub file holds the hex key.
pub = pathlib.Path(".updater/signing.key.pub")
if pub.exists():
    hexkey = pub.read_text().strip().splitlines()[-1].strip()
    try:
        print(base64.b64encode(bytes.fromhex(hexkey)).decode())
        sys.exit(0)
    except Exception:
        pass
sys.exit(1)
PY
)"

if [ -z "${PUB_B64:-}" ]; then
  echo "!! could not read the public key — copy the 'Public key:' line printed above" >&2
  echo "   and paste it into src-tauri/tauri.conf.json -> plugins.updater.pubkey" >&2
  exit 1
fi

python3 - "$PUB_B64" <<'PY'
import json, pathlib, sys
pub = sys.argv[1]
p = pathlib.Path("src-tauri/tauri.conf.json")
cfg = json.loads(p.read_text())
cfg.setdefault("plugins", {}).setdefault("updater", {})["pubkey"] = pub
p.write_text(json.dumps(cfg, indent=2) + "\n")
print("→ wrote public key into src-tauri/tauri.conf.json")
PY

echo
echo "Next steps:"
echo "  1. Add GitHub secrets: TAURI_SIGNING_PRIVATE_KEY (contents of $KEY)"
echo "     and TAURI_SIGNING_PRIVATE_KEY_PASSWORD (the password you chose)."
echo "  2. Tag a release:  git tag v0.2.0 && git push origin v0.2.0"
echo "  3. The workflow builds the AppImage + .sig and publishes latest.json;"
echo "     SharkDeck's Device page then offers 'Install & restart'."
echo
echo "NOTE: the release feed URL must be reachable without auth — make the"
echo "repository public, or point plugins.updater.endpoints at your own host."
