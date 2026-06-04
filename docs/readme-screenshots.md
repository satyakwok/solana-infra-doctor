# README Screenshots

The CLI screenshots in the README (`docs/images/cli/{check,compare,ws}.png`) are
generated from **real `sol-doctor` runs against public Solana endpoints**. They
are not mockups, and the values are not hardcoded.

## What they are (and are not)

- **Real, live runs.** Each image is a real execution captured with color
  enabled. The latency, slot numbers, and slot lag are actual measurements.
- **Not mockups.** No image is hand-edited or assembled from fake data.
- **Not provider endorsements.** Showing an endpoint in a screenshot is not a
  recommendation or a benchmark of that provider.
- **Snapshots, not guarantees.** Values vary by time, region, this VPS's network
  egress, and endpoint conditions. A snapshot is not an SLA or uptime guarantee.

## A monospace font is required

The human output aligns columns with spaces, so the renderer must be displayed
in a **genuinely monospace font**. A proportional font — including a silent
fallback when the requested font is not installed — makes the `Status`,
`Summary`, and `Detail` columns visibly drift even though the underlying text is
correctly aligned.

When capturing, **pin an installed monospace family explicitly** with
`--font.family`. Do not rely on the renderer's default font, and do not use a
user-level `freeze` config that could override it. Verify the family resolves to
itself (not a fallback) before capturing:

```bash
fc-match -f '%{family}\n' "DejaVu Sans Mono"   # must print "DejaVu Sans Mono"
fc-list :spacing=mono family | sort -u          # list installed monospace families
```

## Rules for regenerating them

- **Public endpoints only.** Use endpoints that need no credentials
  (`https://api.mainnet-beta.solana.com`, `https://solana-rpc.publicnode.com`).
- **Never use a private endpoint** or one that requires an API key, token, or
  Basic Auth — these images live in a public repository.
- **No secrets, ever.** URLs are redacted by the CLI, but do not point the tool
  at a URL containing a secret in the first place.
- For `compare`, both endpoints must be on the **same Solana network** (same
  genesis hash). If one is unavailable or rate-limited, substitute another
  verified public mainnet endpoint — do not fake the output.

## How to regenerate

Screenshots are refreshed **manually** (never in CI). Build the release binary,
then capture each command live with `--color always` (so color survives the
non-TTY capture) and an explicit monospace font, writing `docs/images/cli/*.png`:

```bash
cargo build --release
BIN=./target/release/sol-doctor
FONT="DejaVu Sans Mono"   # any installed monospace family (see above)
STYLE=(--font.family "$FONT" --font.size 19 --line-height 1.3 \
       --window --background "#0b0f17" --padding 24 --margin 0 \
       --border.radius 10 --border.width 1 --border.color "#1d2433")

freeze --execute "$BIN check --rpc https://api.mainnet-beta.solana.com --color always" \
  --output docs/images/cli/check.png "${STYLE[@]}"
freeze --execute "$BIN compare --rpc https://api.mainnet-beta.solana.com --rpc https://solana-rpc.publicnode.com --profile bot --color always" \
  --output docs/images/cli/compare.png "${STYLE[@]}"
freeze --execute "$BIN ws --rpc https://api.mainnet-beta.solana.com --color always" \
  --output docs/images/cli/ws.png "${STYLE[@]}"
```

`ws` connects to a live WebSocket and occasionally needs a retry if a single run
is slow to receive the first notification — re-run that one capture; do not edit
the image.

## `freeze` is documentation tooling, not a dependency

`freeze` is used only to render these images locally. It is **not** a runtime or
build dependency of the crate: it is not in `Cargo.toml`, no `freeze` binary or
user config is committed, and nothing in the published crate depends on it. Install
it locally, outside the repository:

```bash
go install github.com/charmbracelet/freeze@v0.2.2
export PATH="$HOME/go/bin:$PATH"
```

The generated PNGs are excluded from the published crate (see `exclude` in
`Cargo.toml`) so they do not bloat the package.
