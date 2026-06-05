# Roadmap

Grounded in usefulness, not feature count. Only shipped functionality is marked
complete. Everything below "Shipped" is a direction, not a promise, and is not
yet available.

## Shipped

- HTTP JSON-RPC readiness (`check`), multi-endpoint comparison (`compare`),
  WebSocket readiness (`ws`), SPL Token / Token-2022 readiness, JSON + Markdown
  reports, credential redaction, workload profiles, CI usage, prebuilt binaries.
- **Yellowstone gRPC readiness check (`grpc check`)** — connect/TLS, optional
  `x-token` auth, safe unary probes, a narrow slot-only `Subscribe` stream with
  time-to-first-update, and an optional HTTP RPC slot cross-check.
- **Yellowstone gRPC endpoint comparison (`grpc compare`)** — rank multiple gRPC
  endpoints by connect latency, time-to-first-event, and slot freshness with
  `general` / `latency` / `indexer` profiles, per-endpoint `x-token` env pairing,
  and JSON + Markdown reports.
- **Data-readiness checks (`check --data`)** — `getProgramAccounts` enablement
  (with an optional `--data-program` override) and archival history depth, for
  indexer and data-pipeline workloads — and **data-readiness scoring in
  `compare --data`** (the `indexer` profile ranks endpoints by it).

## Planned (not yet available)

1. ~~Yellowstone gRPC readiness check~~ — **shipped**.
2. ~~Yellowstone gRPC endpoint comparison~~ — **shipped**.
3. **RPC data-readiness and indexer-readiness** — deeper checks for indexer and
   data-pipeline workloads. Shipped: `getProgramAccounts` enablement + archival
   depth (`check --data`) and indexer-profile scoring (`compare --data`). Next: a
   method-support matrix and `getProgramAccounts` capacity/limit probing.
4. **Local Agave node diagnostics** — inspect a locally reachable validator's
   health and basic configuration.
5. **Agave and Geyser configuration linting** — flag common misconfigurations.
6. **RPC pool and load-balancer consistency checks** — detect divergent
   backends behind a single endpoint.

## Explicitly out of scope

To keep the tool focused, safe, and credible, it does **not** aim to be:

- a hosted monitoring service, dashboard, SaaS, account system, or paid API;
- a security audit, SLA, or long-term provider benchmark;
- a transaction sender or any tool that modifies remote state;
- a broad Yellowstone benchmark, ShredStream/Jito, or Firedancer-specific tool.

These boundaries are intentional and are not on the roadmap.
