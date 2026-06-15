# Add a new RPC check

Single-endpoint HTTP checks live in `src/checks/`. The fastest way in is to copy an
existing check and follow its shape.

## The model

Each probe produces an `RpcCheck` with:

- a **name** (the method or capability, e.g. `getSlot`),
- a **status** (passed / failed / skipped),
- a **latency**, a short **detail** string,
- a **`CheckCategory`** (e.g. `Core`, `Blockhash`, `Performance`, `Token`, `Data`),
- a **`critical`** flag — `true` means a failure forces a `BAD` verdict; `false`
  means it only degrades to `WARNING`.

The overall verdict is computed in `src/checks/verdict.rs` from these flags, so
choosing the right category + criticality is the main design decision.

## Steps

1. **Verify the RPC behaviour first.** Probe the method against a real public
   endpoint (`curl` or the tool) and read the real response shape and error codes —
   RPC semantics are full of provider-specific surprises. Don't design from docs alone.
2. **Add the probe** in the relevant `src/checks/` module. Route the request through
   the existing `RpcClient` so rate-limiting, retries, and URL redaction apply — do
   not build a raw `reqwest` request.
3. **Handle missing / null / malformed fields** and classify transport vs
   application errors via the existing `ErrorKind` taxonomy. Library code returns
   `Result`; it never `panic!`s or exits on bad input.
4. **Push the `RpcCheck`** into the checks list with the right category + `critical`
   flag. Rendering is by category, so human/JSON/Markdown output picks it up for free.
5. **Test it** against the mock HTTP server in `tests/` — a passing case, a
   failing/malformed case, and any edge case. No live-network calls. Keep coverage
   ≥ 95% (drive it from `tests/coverage.rs`, since unit tests are excluded from the
   coverage build).
6. **Document** it: add it to `## What It Checks` / `## Check Details` in the README
   if user-facing, and a `CHANGELOG.md` entry.

## Don't

- Don't use a huge program (e.g. the SPL Token program) as a `getProgramAccounts`
  probe target — large programs are excluded from validator secondary indexes and
  return error `-32010` on most providers. Use a small, non-excluded program.
- Don't make the check non-deterministic or dependent on wall-clock timing.
