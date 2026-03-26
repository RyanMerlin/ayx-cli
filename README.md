# AYX CLI

Rust-first workspace scaffold for the `ayx` CLI.

## Current status
- Workspace and crate structure created from the migration plan.
- Top-level `ayx mongo` and `ayx api` command trees are wired to the shared envelope and profile loader.
- Dry-run/apply safety model, audit artifacts, and managed/embedded Mongo tooling scaffolds exist.
- API operations cover governance-heavy endpoints such as schedules, collections, DCM/credentials, user-groups, subscriptions, and workflow ownership changes backed by Swagger-derived contracts.
- `ayx update` now hooks into GitHub releases and the Windows release workflow so the `ayx` binary can refresh itself via `self_update`.

## Next
- Replace the Mongo orchestration stubs with production-grade AlteryxGallery/AlteryxService clients (full parity with the Python tooling).
- Expand the API coverage (schedules, collections, DCM, owner transfer edge cases) and harden contract tests around the Swagger definitions.
- Add integration pipelines and multi-platform release automation (Windows/Linux/macOS).

## Binary & release

The workspace exposes a single binary called `ayx`. The GitHub Actions workflow at `.github/workflows/build-release.yml` runs on each push to `main` and every new `v*` tag to build the release artifacts on Windows (the base platform for Server deployments).

When cutting a release, publish the compiled `ayx` executable (and optional archives for other hosts) under a GitHub tag so the `ayx update` command can find the asset that matches the local target triple.

## Self-update

`ayx update [--repo-owner <owner>] [--repo-name <repo>] [--bin-name <name>] [--target-version <tag>] [--skip-confirm]` uses the `self_update` crate to fetch a GitHub release asset that matches the running target triple, swaps in the new binary, and reports success through the envelope model. The defaults are `RyanMerlin/ayx-cli` for the release and `ayx` for the binary name, which keeps the upgrade path aligned with the hosted repo.

Use `--target-version` to install a specific release and `--skip-confirm` for automation. Mutating commands still require `--apply` so auto updates never circumvent the existing safety gates.

## Quick install

The most reliable one-line install for this workspace is to build and install directly from source:

```powershell
cargo install --locked --path .
```

That produces the `ayx` binary in Cargo's global bin directory and keeps the install reproducible from the checked-in workspace. If you prefer a release-based install on Unix-like shells, the repo also includes `scripts/install.sh`:

```bash
curl -fsSL https://raw.githubusercontent.com/RyanMerlin/ayx-cli/main/scripts/install.sh | bash
```

The release script honors `AYX_VERSION` for a fixed tag and `AYX_INSTALL_DIR` for a custom install path. The same release assets power `ayx update`, so once the CLI is installed the updater can keep it current.

## Configuration

`ayx` loads its profile from `config.yaml` in the working directory by default. The current sample covers both embedded and managed Mongo scenarios, API OAuth2 client credentials, and the required Alteryx One account email. Keep secrets out of source control by replacing placeholders with Vault/KeyVault references in production.

### Mongo settings

- `mongo.mode` chooses between `embedded` (auto-discover RuntimeSettings, `AlteryxService.exe` wrappers, and the embedded `emongodump/emongorestore` hooks) and `managed` (external `mongodump/mongorestore` tools).
- `mongo.databases` names the `AlteryxGallery` and `AlteryxService` databases so every command knows which namespaces to touch.
- When `mongo.mode` is `embedded`, leave `mongo.embedded.runtime_settings_path` null and the CLI will probe `C:\ProgramData\Alteryx\RuntimeSettings.xml`, `%ProgramData%/Alteryx/…`, `%ProgramFiles%/Alteryx/…`, `%ProgramFiles(x86)%/Alteryx/…`, or relocated drives (e.g., `D:\ProgramData/Alteryx/RuntimeSettings.xml`) before asking you to override it manually. `restore_target_path` and `alteryx_service_path` are optional overrides derived from the runtime payload.
- In `managed` mode, provide either `mongo.managed.url` or `host`+`port`. TLS fields (`enabled`, `ca_path`, `cert_path`, `key_path`, `allow_invalid_hostnames`) control how `mongodump/mongorestore` authenticate, and timeout/retry/pool knobs tune the driver's resilience.

### API settings

- `api.base_url` points at the Server web API root (for example `http://172.27.171.32/webapi/`).
- OAuth2 client credentials are configured under `api.auth` (`client_id`, `client_secret`, optional `scope`). The CLI derives `${api.base_url}oauth2/token` automatically.
- `api.timeout_ms` keeps HTTP calls responsive while retaining the envelope data model for replay/debug.

### Alteryx One

- `alteryx_one.account_email` stores the Alteryx One username used across gallery operations and the owner-transfer automation.

### Runtime settings fixture

The repo mirrors a real `RuntimeSettings.xml` at `C:\code\RuntimeSettings.xml` for offline experimentation. Copy a live Server runtime settings file there (or point `mongo.embedded.runtime_settings_path` at an existing install) to exercise the embedded discovery logic.
