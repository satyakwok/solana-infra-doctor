#!/usr/bin/env bash
#
# Capture the README CLI screenshots from REAL sol-doctor runs against public
# Solana endpoints. This is local documentation tooling — it is not run in CI
# and `freeze` is not a project dependency.
#
# The images it writes (docs/images/cli/*.png) are real diagnostic snapshots:
# live values vary by time, region, and endpoint conditions. Never point this at
# a private endpoint or an endpoint that needs an API key — the images are
# committed to a public repository.
#
# Requirements:
#   - a release build of sol-doctor (built here)
#   - `freeze` (https://github.com/charmbracelet/freeze) on PATH or in ~/go/bin
#
# Usage:
#   ./scripts/capture-readme-screenshots.sh

set -euo pipefail

if [[ -n "${CI:-}" ]]; then
  echo "error: this script captures live screenshots and must not run in CI." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Public endpoints only. Both must be on the same Solana network (same genesis
# hash) for the comparison to rank; if one is unavailable, substitute another
# verified public mainnet endpoint rather than faking output.
RPC_PRIMARY="https://api.mainnet-beta.solana.com"
RPC_SECONDARY="https://solana-rpc.publicnode.com"

out_dir="docs/images/cli"
mkdir -p "$out_dir"

# Locate freeze (PATH, then the conventional Go bin dir).
if command -v freeze >/dev/null 2>&1; then
  FREEZE="$(command -v freeze)"
elif [[ -x "$HOME/go/bin/freeze" ]]; then
  FREEZE="$HOME/go/bin/freeze"
else
  echo "error: 'freeze' not found. Install it locally (outside this repo):" >&2
  echo "  go install github.com/charmbracelet/freeze@v0.2.2" >&2
  echo "  export PATH=\"\$HOME/go/bin:\$PATH\"" >&2
  exit 1
fi

echo "Building release binary..."
cargo build --release --quiet
BIN="$repo_root/target/release/sol-doctor"
echo "sol-doctor version: $("$BIN" --version)"

# Shared freeze styling: a clean dark terminal window, readable monospace.
freeze_style=(
  --window
  --background "#0b0f17"
  --padding 24
  --margin 0
  --border.radius 10
  --border.width 1
  --border.color "#1d2433"
  --font.size 19
  --line-height 1.3
)

capture() {
  local name="$1"
  shift
  echo "Capturing $name (live)..."
  # --color always so color survives the non-TTY capture; output is real.
  "$FREEZE" --execute "$* --color always" \
    --output "$out_dir/$name.png" \
    "${freeze_style[@]}"
}

capture check "$BIN check --rpc $RPC_PRIMARY"
capture compare "$BIN compare --rpc $RPC_PRIMARY --rpc $RPC_SECONDARY --profile bot"
capture ws "$BIN ws --rpc $RPC_PRIMARY"

echo
echo "Wrote:"
ls -1 "$out_dir"/*.png
echo
echo "Note: values are live and vary per run. Verify no secrets or private URLs"
echo "appear before committing (public endpoints only)."
