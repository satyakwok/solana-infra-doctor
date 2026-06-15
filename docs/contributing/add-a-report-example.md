# Add an example / terminal output

Examples make the tool understandable at a glance. Two kinds live in the repo.

## Text examples (`examples/terminal/`)

Plain captures of real runs against **public** endpoints, with a header noting the
exact command, UTC timestamp, version, and endpoint label. Generate with
`--color never` for byte-stable output:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --color never > out.txt
```

Then prepend the header (see the existing files for the format) and save under
`examples/terminal/`.

## Screenshots (`docs/images/cli/terminal/`)

Real-terminal captures, regenerated manually (never in CI) via the documented
`Xvfb` + `xterm` + `scrot` + ImageMagick pipeline in
[`docs/terminal-screenshots.md`](../terminal-screenshots.md). Add a gallery entry in
that file. `docs/images/` is excluded from the published crate.

## Rules

- **Public endpoints only.** Never a private/credentialed endpoint, never a secret.
- **Real output only.** No mockups, no hand-edited or assembled data.
- **Yellowstone gRPC:** a *successful* run cannot be shown publicly because every
  provider needs a private `x-token`. Use a non-resolving placeholder host
  (`*.example.com`) to demonstrate error handling instead.
- Numbers are point-in-time and will drift; that's expected.
