# Security Policy

## Supported Versions

Solana Infra Doctor is currently pre-release. Security fixes target the default branch until stable releases are published.

## Reporting a Vulnerability

Please report security issues privately through GitHub security advisories for this repository when available. If advisories are unavailable, contact the repository owner with a minimal description and reproduction details.

Do not include sensitive RPC credentials, private endpoint URLs, or production secrets in public issues.

## Handling Sensitive Data

The CLI redacts credentials and query strings from displayed RPC URLs. Contributors should avoid adding logs or diagnostics that expose secrets.
