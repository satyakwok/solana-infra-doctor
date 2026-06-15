# Add a redaction-safe real-run report

Real runs against real endpoints make the tool trustworthy. They also need care:
**a report must never contain a secret.**

## Generate a Markdown report

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot \
  --report report.md
```

`check` and `grpc check` also support `--report`.

## Redaction rules

- `sol-doctor` redacts credentials in URLs (basic-auth, `?api-key=…`, path tokens)
  and drops the entire query string. Still, **review the file before committing.**
- For a **private** endpoint: only commit it if the URL is fully redacted and the
  endpoint is referred to by a generic label (e.g. "private provider"). When in
  doubt, use a public endpoint instead.
- **Yellowstone gRPC tokens** are read only from an env var via `--x-token-env` —
  never the command line, never printed, never committed.
- Run a final check: search the file (and the whole diff) for anything resembling a
  key or token before opening the PR.

## Framing (honesty)

- Add a header caveat: the report is a **point-in-time** snapshot from a single
  location against the named endpoints; latency and freshness vary by time, region,
  and load.
- It is **not** an authoritative provider ranking and **not** an SLA. Scores are
  deterministic heuristics, not a guarantee of provider behaviour.
- Don't imply any provider is "bad"; frame differences as fit-for-workload.

## Where it goes

Committed reports live in `examples/reports/`. See the existing files (including the
multi-provider example) for the expected shape.
