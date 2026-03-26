# AYX CLI

Rust-first workspace scaffold for the `ayx` CLI.

## Current status
- Workspace and crate structure created from the migration plan.
- Top-level `ayx mongo` and `ayx api` command trees are wired to the shared envelope and profile loader.
- Dry-run/apply safety model, audit artifacts, and managed/embedded Mongo tooling scaffolds exist.
- API operations now cover schedule detail/delete/patch (guarded by `--apply`), schedule create/update (payload files with `CreateScheduleContract`/`UpdateScheduleContract`), collection detail/create/delete/update (with payload files for updates), credential share/unshare for users and user groups (via `AddCredentialsUserContract`/`AddCredentialsUserGroupContract`), user-group CRUD/membership management (via `CreateUserGroupContract`/`UpdateUserGroupContract`), DCM connection lookup/shares, DCM admin connection list/upsert/delete/share removal helpers, and the prior status/users/workflows/collections/schedules/DCM listing commands against the Swagger-driven endpoints.

## Next
- Replace the Mongo orchestration stubs with production-grade AlteryxGallery/AlteryxService clients (full parity with the Python tooling).
- Expand the API coverage (schedules, collections, DCM, owner transfer edge cases) and harden contract tests around the Swagger definitions.
- Add integration pipelines and multi-platform release automation (Windows/Linux/macOS).

## Configuration

`ayx` loads its profile from `config.yaml` in the working directory by default. The current sample covers both embedded and managed Mongo scenarios, API OAuth2 client credentials, and the required Alteryx One account email. Keep secrets out of source control by replacing placeholders with Vault/KeyVault references in production.

### Mongo settings

- `mongo.mode` chooses between `embedded` (Auto-discover RuntimeSettings, `AlteryxService.exe` wrappers, and emongodump/emongorestore) and `managed` (external `mongodump/mongorestore` tools).
- `mongo.databases` names the `AlteryxGallery` and `AlteryxService` databases so every command knows which namespaces to touch.
- When `mongo.mode` is `embedded`, leave `mongo.embedded.runtime_settings_path` null and the CLI will probe `C:\ProgramData\Alteryx\RuntimeSettings.xml`, `%ProgramData%/Alteryx/…`, `%ProgramFiles%/Alteryx/…`, `%ProgramFiles(x86)%/Alteryx/…`, and relocated drives (e.g., `D:\ProgramData`) before asking you to override it manually.
- `mongo.embedded.alteryx_service_path` and `mongo.embedded.restore_target_path` are optional overrides; the runtime settings file can usually derive the right install and persistence paths.
- In `managed` mode, provide either `mongo.managed.url` or `host`+`port`. TLS fields (`enabled`, `ca_path`, `cert_path`, `key_path`, `allow_invalid_hostnames`) control how `mongodump/mongorestore` connect, and timeout/retry/pool knobs tune the client's resilience.

### API settings

- `api.base_url` points at the Server web API root (for example `http://172.27.171.32/webapi/`).
- OAuth2 client credentials are configured under `api.auth` (`client_id`, `client_secret`, optional `scope`). The CLI derives `${api.base_url}oauth2/token` automatically.
- `api.timeout_ms` keeps HTTP calls responsive while retaining the envelope data model for replay/debug.

### Alteryx One

- `alteryx_one.account_email` stores the Alteryx One username used across gallery operations and the owner-transfer automation.

### Runtime settings fixture

The repo mirrors a real `RuntimeSettings.xml` at `C:\code\RuntimeSettings.xml` for offline experimentation. Copy a live Server runtime settings file there (or point `mongo.embedded.runtime_settings_path` at an existing install) to exercise the embedded discovery logic.
