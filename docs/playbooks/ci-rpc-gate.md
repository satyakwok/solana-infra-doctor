# Playbook: gating CI on RPC readiness

## Problem

You want CI to fail fast if the RPC your tests/deploys depend on is unhealthy —
before a flaky endpoint wastes a pipeline or ships against bad infra.

## Recommended command

`sol-doctor` exit codes map cleanly to CI: `0` GOOD · `1` WARNING · `2` BAD · `3`
UNKNOWN. A non-zero exit fails the step.

```bash
sol-doctor check --rpc "$RPC_URL" --fail-on-warning
```

`--fail-on-warning` makes a `WARNING` verdict also fail the job (strict mode).

### GitHub Actions (via the action wrapper)

```yaml
- uses: satyakwok/solana-infra-doctor@v1
  with:
    command: check
    rpc: ${{ secrets.RPC_URL }}
    fail-on-warning: "true"
    json: "true"
```

The action installs the prebuilt binary (no compile) and runs the command; the job
fails on a non-zero verdict.

## What to look at

- The **exit code** drives the gate; the **verdict line** explains it.
- Use `--json` to archive a machine-readable record (it carries `schema_version`).
- For multi-endpoint gating, run `compare` and inspect the best/worst scores.

## Common failure cases

- `UNKNOWN` (exit 3) — not enough data (e.g. the endpoint was unreachable); treat as
  a failure.
- `WARNING` without `--fail-on-warning` exits `1` only in strict mode — decide which
  you want.
- A private `RPC_URL` in logs — pass it via a **secret**, never inline; the tool
  redacts URLs in its own output.

## Notes & safety

- Store endpoints/keys in CI **secrets**; the action reads `rpc` from the input.
- Output is point-in-time — a green gate means "healthy now," not an SLA.
