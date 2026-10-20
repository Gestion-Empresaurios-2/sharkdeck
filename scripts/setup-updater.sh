#!/usr/bin/env sh
# One-time updater setup for SharkDeck.
#
# It does the sensitive part ON YOUR MACHINE (a private signing key is never
# handed to an agent or committed): generates a Tauri updater keypair, writes the
# public key into tauri.conf.json, and stores the private key in .updater/ for
# the release build.
#
#   sh scripts/setup-updater.sh        # works with dash/sh AND bash
#   ./scripts/setup-updater.sh
#
# Afterwards: add the printed TAURI_SIGNING_PRIVATE_KEY / _PASSWORD as GitHub
# repository secrets so the release workflow can sign update artifacts.
set -eu
# pipefail is a bash/ksh/zsh feature; enable it only when the shell has it.
(set -o pipefail) 2>/dev/null && set -o pipefail || true

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

KEY_DIR="$ROOT/.updater"
KEY="$KEY_DIR/signing.key"
mkdir -p "$KEY_DIR"

if [ -f "$KEY" ]; then
  echo "→ key already exists at $KEY (delete it to rotate)"
else
  echo "→ generating updater keypair (choose a password — you will need it in CI)"
  # The CLI's flag has moved around: v2 used `--writable-keys`, newer builds use
  # `--write-keys <PATH>` (short `-w`). The short form is stable across both, so
  # prefer it and keep the long spellings as fallbacks.
  PASS_ARGS=""
  if [ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
    PASS_ARGS="-p $TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
  fi
  gen_ok=0
  # shellcheck disable=SC2086
  if npx tauri signer generate -w "$KEY" $PASS_ARGS; then
    gen_ok=1
  else
    echo "→ local tauri CLI failed, trying the published one"
    # shellcheck disable=SC2086
    if npx --yes @tauri-apps/cli@latest signer generate -w "$KEY" $PASS_ARGS; then
      gen_ok=1
    else
      echo "→ trying the legacy flag spelling"
      # shellcheck disable=SC2086
      if npx --yes @tauri-apps/cli@latest signer generate --writable-keys -w "$KEY" $PASS_ARGS; then
        gen_ok=1
      fi
    fi
  fi
  if [ "$gen_ok" -ne 1 ]; then
    echo "!! key generation failed — run it by hand:" >&2
    echo "   npx tauri signer generate -w $KEY" >&2
    exit 1
  fi
fi

PUB_B64="$(python3 - <<'PY'
import base64, pathlib, sys
# The CLI prints the base64 public key; the .pub file next to the private key
# holds the hex key. Accept any *.pub inside .updater as a fallback.
cands = sorted(pathlib.Path(".updater").glob("*.pub"))
if not cands:
    sys.exit(1)
for pub in cands:
    hexkey = pub.read_text().strip().splitlines()[-1].strip()
    try:
        print(base64.b64encode(bytes.fromhex(hexkey)).decode())
        sys.exit(0)
    except Exception:
        continue
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
echo "     From this folder:  gh secret set TAURI_SIGNING_PRIVATE_KEY < $KEY"
echo "                         gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
echo "  2. Tag a release:  git tag v0.2.0 && git push origin v0.2.0"
echo "  3. The workflow builds the AppImage + .sig and publishes latest.json;"
echo "     SharkDeck's Device page then offers 'Install & restart'."
echo
echo "NOTE: the release feed URL must be reachable without auth — make the"
echo "repository public, or point plugins.updater.endpoints at your own host."

# Optional: push the secrets straight into the GitHub repo (no key on screen).
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  echo
  read -r -p "→ set the GitHub secrets now? (private key + password) [y/N] " ans || ans=""
  case "$ans" in
    [yY]|[yY][eE][sS])
      gh secret set TAURI_SIGNING_PRIVATE_KEY < "$KEY"
      gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
      echo "→ secrets uploaded to $(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || echo 'the repo')"
      ;;
    *)
      echo "→ skipped; set them later with the commands above"
      ;;
  esac
fi
