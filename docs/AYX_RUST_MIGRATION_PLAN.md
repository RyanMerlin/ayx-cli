# AYX Rust CLI Re-Architecture Plan (Mongo-First Full Parity v1)

## Summary
Build `C:\code\ayx-cli` as a Rust-first, production-grade `ayx` CLI with full parity to `alteryx-omni` and `ayxm`, with immediate Python cutover.
MongoDB is a first-class subsystem and prioritized for v1: both embedded Alteryx Server Mongo and user-managed external Mongo must be fully supported.

## Implementation Changes
- Rust workspace crates:
  - `ayx-cli`, `ayx-core`, `ayx-server`, `ayx-api`, `ayx-mongo`, `ayx-sqlserver`, `ayx-workflow`, `ayx-cloud`, `ayx-docs-schema`.

- Mongo-first architecture in `ayx-mongo`:
  - Explicit connection modes:
    - `embedded` (discover from RuntimeSettings.xml and Alteryx install context)
    - `managed` (external URI/host/port with full user control)
  - Support both Alteryx databases: `AlteryxGallery`, `AlteryxService`.
  - Connection/auth support:
    - SCRAM-SHA-256 credentials
    - direct URI mode for externally managed instances
    - TLS options, CA path, cert/key, hostname validation flags
    - timeout/retry/pool sizing controls
  - Feature parity for existing Mongo operations:
    - inventory/query operations
    - auth-transition read/write flows
    - workflow ownership and migration-related updates
    - backup/restore helper orchestration
- Safety model:
  - all mutating Mongo commands default to dry-run
  - `--apply` required for writes
    - operation audit artifact output for every mutation command

- CLI surface and interfaces:
  - `ayx mongo ...` promoted to top-level first-class domain (not a secondary helper).
  - Unified profile schema (`profiles.v2.yaml`) includes:
    - `mongo.mode: embedded|managed`
    - `mongo.embedded.runtime_settings_path`
    - `mongo.managed.uri` and/or host/port/auth fields
    - TLS/security and per-DB credential references
  - Auto-migrate legacy plaintext secrets to secure references (per prior policy).
  - Stable JSON envelope for all commands plus generated `docs/cli-schema.json` and `docs/cli-spec.md`.

- Platform/release:
  - Windows/Linux/macOS release artifacts.
  - Signed binaries, SBOM, reproducible build pipeline.

## Test Plan
- Mongo-focused test tiers (required for release):
  - Unit: connection parsing, embedded discovery, auth/TLS option handling.
  - Integration: live tests against
    - embedded-style local Mongo config
    - externally managed Mongo instance
    - both `AlteryxGallery` and `AlteryxService`.
  - Mutation safety: verify dry-run default and `--apply` gate enforcement.
  - Parity: golden comparisons against Python outputs for all existing Mongo-related workflows.
- Broader parity/integration/e2e tests remain mandatory across server/api/sql/workflow/cloud domains.

## Assumptions and Defaults
- `ayx` command name is final.
- v1 is full hard parity across both Python repos.
- immediate Python cutover after Rust parity validation.
- MongoDB is the top-priority subsystem and release blocker.
- SQL support in v1 is Microsoft SQL Server only.
- Cloud auth baseline remains OAuth2 client credentials + PAT.
