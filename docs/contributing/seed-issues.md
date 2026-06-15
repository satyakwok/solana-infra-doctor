# Seed issues

Ready-to-open issues for maintainers and contributors. Copy a block into a new
GitHub issue. Each is scoped and has acceptance criteria. **Before claiming
something is "missing," check the current behaviour** — frame work as add/improve,
and verify against the code first.

Standard acceptance gates (apply to every code issue):
`cargo fmt --all -- --check` · `cargo clippy --all-targets --all-features -- -D warnings`
· `cargo test --all-features` · coverage ≥ 95% · docs/CHANGELOG updated if user-facing.

---

## Good first issues (8)

### 1. Add an informational `getEpochInfo` check
**Labels:** `good first issue`, `enhancement`
**Context:** The `Core` category reports basic chain state. Epoch progress is a
useful readiness signal. Verify it isn't already covered, then add it as a
non-critical check (a failure should degrade to `WARNING`, not `BAD`).
**Task:** Add the probe in `src/checks/`, route it through `RpcClient`, push an
`RpcCheck` in the `Core` category with `critical = false`.
**Acceptance:** Pass + malformed-response tests against the mock server; rendered in
human/JSON output; standard gates.

### 2. List exit codes in `sol-doctor --help`
**Labels:** `good first issue`, `docs`
**Context:** Exit codes (`0` GOOD / `1` WARNING / `2` BAD / `3` UNKNOWN) are in the
README but not in `--help`.
**Task:** Add an `after_help`/epilog to the clap command listing them.
**Acceptance:** `sol-doctor --help` shows the exit codes; gates pass.

### 3. Make the invalid-URL error suggest the expected scheme
**Labels:** `good first issue`, `enhancement`
**Context:** `AppError::InvalidRpcUrl` reports the reason but doesn't hint the fix.
**Task:** Include the accepted schemes (`http(s)://`, `ws(s)://`) in the message.
**Acceptance:** A test asserts the improved message; gates pass.

### 4. Reject `--samples 0` with a clear error
**Labels:** `good first issue`, `enhancement`
**Context:** `--samples` drives repeat latency sampling. Confirm how `0` behaves
today; if it's a no-op or panic, validate it.
**Task:** Validate `--samples >= 1` and return a clear error otherwise.
**Acceptance:** Test covers the rejection path; gates pass.

### 5. Add a regression test for the elevated-latency `WARNING` path
**Labels:** `good first issue`, `tests`
**Context:** `calculate_verdict` caps the verdict at `WARNING` when average latency
is in the elevated band. This branch deserves an explicit test.
**Task:** Add a `tests/coverage.rs` case that builds a report in the elevated-latency
band and asserts `WARNING`.
**Acceptance:** New test passes; coverage holds.

### 6. Add a redaction test for a query-param token
**Labels:** `good first issue`, `tests`, `security`
**Context:** `src/redact.rs` drops the whole query string. Lock that with a test for
a realistic param name.
**Task:** Add a test asserting `?access_token=SECRET` is removed from the rendered URL.
**Acceptance:** Test passes. **Safety:** use a fake token literal only.

### 7. Assert `compare --json` never emits ANSI codes
**Labels:** `good first issue`, `tests`
**Context:** JSON output must be machine-clean.
**Task:** Add a test asserting `render_compare_json` output contains no `\x1b`.
**Acceptance:** Test passes; gates pass.

### 8. Name the `--x-token-env` flag in the missing-token error
**Labels:** `good first issue`, `enhancement`
**Context:** `AppError::MissingTokenEnv` names the env var; it could also remind the
user how it's passed.
**Task:** Mention `--x-token-env` in the message.
**Acceptance:** Test asserts the message; the token value is never included.

---

## Documentation issues (5)

### 9. Add a troubleshooting section
**Labels:** `docs`
**Context:** Users hit `429 Too Many Requests`, timeouts, and "method not available
on free tier" responses.
**Task:** Add a troubleshooting doc covering these, what they mean, and what to try.
**Acceptance:** New doc linked from the README.

### 10. Expand the JSON schema reference
**Labels:** `docs`
**Context:** `docs/cli-output.md` describes JSON output but not every field.
**Task:** Document the `check`/`compare`/`ws` JSON fields, including `schema_version`.
**Acceptance:** Reference matches actual `--json` output.

### 11. Add a glossary
**Labels:** `docs`, `good first issue`
**Context:** Terms like *slot lag*, *freshness*, *archival depth*, *secondary index*
recur without definitions.
**Task:** Add `docs/glossary.md`; link from the README.
**Acceptance:** Each term defined in one or two sentences.

### 12. Indonesian parity for a playbook
**Labels:** `docs`, `help wanted`
**Context:** The README has an Indonesian version (`README.id.md`); the playbooks
don't.
**Task:** Translate one `docs/playbooks/*.md` to Indonesian.
**Acceptance:** Accurate translation; same commands/caveats.

### 13. Document the GitHub Action inputs end-to-end
**Labels:** `docs`
**Context:** `action.yml` exposes inputs; a worked CI example helps adoption.
**Task:** Add a docs section with a complete workflow snippet that gates a job on the
verdict.
**Acceptance:** Snippet matches the current action inputs.

---

## Help wanted (4)

### 14. Baseline / drift mode
**Labels:** `help wanted`, `enhancement`
**Context:** Teams want "did my provider quietly degrade?" — comparing a current run
to a saved baseline. This is on the roadmap.
**Task:** Design and implement a baseline file (saved `--json`) and a diff that flags
regressions. Keep it deterministic and local-first.
**Acceptance:** Tests for improved/regressed/unchanged; docs; coverage holds.

### 15. Distinguish provider-gated responses from real failures
**Labels:** `help wanted`, `enhancement`
**Context:** Some public endpoints return `4xx` like "method not available on free
tier" — that's a gating signal, not a transport failure, and should be classified
distinctly.
**Task:** Extend the `ErrorKind` taxonomy and verdict/summary text to surface gating
clearly. **Verify real responses against live endpoints first.**
**Acceptance:** Deterministic tests for the new classification; gates pass.

### 16. Machine-readable CI summary from the Action
**Labels:** `help wanted`, `enhancement`
**Context:** CI users want structured outputs (verdict, score) without parsing text.
**Task:** Emit `GITHUB_OUTPUT` values from the Action wrapper, documented.
**Acceptance:** Action self-test exercises it; docs updated.

### 17. Improve the `--data` `getProgramAccounts` probe
**Labels:** `help wanted`, `enhancement`
**Context:** `--data` probes `getProgramAccounts` enablement on a small program.
There may be more robust ways to detect gating/limits across providers.
**Task:** Research provider behaviours (live), then improve the probe/classification
without using an excluded program (e.g. SPL Token) as the target.
**Acceptance:** Tests; documented rationale; coverage holds.

---

## Real-run / provider testing (3)

### 18. Multi-provider `--data` data-readiness report
**Labels:** `real-run`, `help wanted`
**Context:** Archival depth and `getProgramAccounts` enablement differ a lot by
provider.
**Task:** Run `check --data` against 3+ **public** endpoints and post a redacted
report of the differences.
**Acceptance:** Real, redacted output; point-in-time caveat. **Safety:** public
endpoints only; no keys/tokens.

### 19. Credentialed `grpc check` degraded-detection report
**Labels:** `real-run`, `help wanted`
**Context:** We want evidence that `grpc check` correctly detects degraded
Yellowstone endpoints.
**Task:** Run `grpc check` against your own credentialed provider and post the
**redacted** result + whether the verdict matched reality.
**Acceptance:** No token/URL leaked (`--x-token-env` only); honest framing.

### 20. Region-sensitivity check on `compare`
**Labels:** `real-run`, `help wanted`
**Context:** Latency is region-dependent; rankings may flip by location.
**Task:** Run the same `compare` from a non-US region and report whether the winner
changes.
**Acceptance:** Redacted, public-endpoint output; note the region (not your IP).
