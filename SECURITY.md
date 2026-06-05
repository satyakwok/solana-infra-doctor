# Security Policy

## Supported Versions

Solana Infra Doctor is published to [crates.io](https://crates.io/crates/solana-infra-doctor) and is pre-1.0, so it follows semantic versioning with the `0.x` convention (minor releases may include breaking changes). The **latest published release** is the supported version; older releases are not maintained. Security fixes land on the default branch (`main`) and ship in the next release.

## Reporting a Vulnerability

Please report security issues privately through GitHub security advisories for this repository when available. If advisories are unavailable, contact the repository owner with a minimal description and reproduction details.

Do not include sensitive RPC credentials, private endpoint URLs, or production secrets in public issues.

## Handling Sensitive Data

The CLI redacts credentials and query strings from displayed RPC URLs. Contributors should avoid adding logs or diagnostics that expose secrets.
