# Glossary

## slot lag

In a `compare` run, how many slots an endpoint trails the freshest endpoint
in that run. A lag of `0` means this endpoint is the baseline. Not computed
across different networks (genesis hash mismatch).

## freshness

A measure of data recency evaluating how current an endpoint's slot stream is (slot freshness) or how far behind the actual wall-clock time its finalized block data lags (block-time freshness).

## archival depth

The historical range or age limit of blockchain data that a specific node or endpoint stores and makes available for querying.

## secondary index

An additional database index created on non-primary fields to allow efficient querying of blockchain data by attributes other than the main identifier.

## verdict

The final classification or decision determined by a system regarding the state, validity, or health of a specific node or data payload.

## workload profile

A named preset selected via `--profile` that adjusts how `compare` scores and
ranks endpoints. Each profile (`general`, `wallet`, `bot`, `indexer`, `ci`)
applies different weights to latency, freshness, and capability signals to
match a specific use case.

## data-readiness

Optional capability probes enabled by the `--data` flag: checking whether
`getProgramAccounts` responds for a target program, and how far back the
endpoint's archival history reaches via `getFirstAvailableBlock`. Off by
default; informational only.

## gated

A `getProgramAccounts` probe result: the endpoint answered the request, but
the method is unavailable for the probed program — either the RPC node
disables the method, or the queried program is excluded from the account
secondary indexes.

## genesis hash

The unique cryptographic hash of a blockchain's very first block (Block 0), which serves as a permanent identifier for that specific network.

## Yellowstone gRPC

A high-performance, low-latency streaming framework designed to deliver real-time Solana ledger data to indexers and applications.

## x-token

An authentication token passed in API headers to verify a client's identity and authorize their access to protected endpoints or data streams.

## block time

The specific timestamp or time interval required for a network to produce, validate, and append a new block to the blockchain.

## exit code

A standardized numeric status code returned by a process or container upon completion to indicate whether it executed successfully or encountered a specific error.
