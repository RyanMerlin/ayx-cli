use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use ayx_core::definitions::DEFAULT_RUNTIME_SETTINGS_PATH;
use ayx_core::envelope::Envelope;
use ayx_core::profile::{Config, ServerProfile};
use ayx_server::logs::{
    discover_log_inventory, extract_context, parse_gallery_csv, parse_gallery_events,
    parse_service_events, recent_log_candidates, summarize_log_file, tail_log_file,
};
use ayx_server::mongo::{backup_envelope, inventory_envelope, restore_envelope, status_envelope};
use ayx_server::upgrade::{
    compute_path, run_apply, run_backup, run_bundle, run_plan, run_postcheck, run_precheck,
};
use ayx_server::util::{
    ayx_paths, backup_plan, capture_system_info, run_server_backup, runtime_settings_summary,
    write_runtime_settings_json,
};
use ayx_server::{call_operation, import_swagger};
use self_update::backends::github::Update as GitHubUpdate;
use self_update::Status;

#[derive(Parser, Debug)]
#[command(name = "ayx")]
#[command(about = "AYX Rust CLI")]
struct Cli {
    #[arg(long, default_value = "text")]
    output: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Mongo {
        #[command(subcommand)]
        command: MongoCommand,
    },
    Api {
        #[command(subcommand)]
        command: ApiCommand,
    },
    Server {
        #[command(subcommand)]
        command: Option<ServerCommand>,
    },
    Upgrade {
        #[command(subcommand)]
        command: UpgradeCommand,
    },
    Sqlserver {
        #[command(subcommand)]
        command: Option<SqlserverCommand>,
    },
    Workflow {
        #[command(subcommand)]
        command: Option<WorkflowCommand>,
    },
    Cloud {
        #[command(subcommand)]
        command: Option<CloudCommand>,
    },
    Update {
        #[arg(long, default_value = "RyanMerlin")]
        repo_owner: String,
        #[arg(long, default_value = "ayx-cli")]
        repo_name: String,
        #[arg(long, default_value = "ayx")]
        bin_name: String,
        #[arg(long)]
        target_version: Option<String>,
        #[arg(long)]
        skip_confirm: bool,
    },
}

#[derive(Subcommand, Debug)]
enum MongoCommand {
    Status {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
    },
    Inventory {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
    },
    Backup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "backups")]
        output_dir: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long, default_value = "audits")]
        audit_dir: PathBuf,
    },
    Restore {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        input_path: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long, default_value = "audits")]
        audit_dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum ApiCommand {
    Status {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
    },
    Users {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "Default")]
        view: String,
    },
    UserDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_id: String,
    },
    UserUpdate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    UserDelete {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        apply: bool,
    },
    UserAssets {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        asset_type: Option<String>,
    },
    UserTransferAssets {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    UserDeactivate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        apply: bool,
    },
    UserPasswordReset {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        apply: bool,
    },
    Workflows {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "Default")]
        view: String,
    },
    WorkflowDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        workflow_id: String,
    },
    WorkflowJobs {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        workflow_id: String,
    },
    WorkflowQuestions {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        workflow_id: String,
        #[arg(long)]
        version_id: Option<String>,
    },
    WorkflowPackage {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        workflow_id: String,
        #[arg(long)]
        version_id: Option<String>,
        #[arg(long)]
        output_path: Option<PathBuf>,
    },
    WorkflowVersionUpload {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        workflow_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        owner_id: String,
        #[arg(long)]
        file_path: PathBuf,
        #[arg(long, default_value_t = false)]
        others_may_download: bool,
        #[arg(long, default_value_t = false)]
        others_can_execute: bool,
        #[arg(long, default_value = "Safe")]
        execution_mode: String,
        #[arg(long, default_value_t = false)]
        has_private_data_exemption: bool,
        #[arg(long)]
        comments: Option<String>,
        #[arg(long, default_value_t = false)]
        make_published: bool,
        #[arg(long, default_value = "Default")]
        workflow_credential_type: String,
        #[arg(long)]
        credential_id: Option<String>,
        #[arg(long, default_value_t = false)]
        bypass_workflow_version_check: bool,
        #[arg(long)]
        apply: bool,
    },
    Schedules {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "Default")]
        view: String,
    },
    ScheduleDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        schedule_id: String,
    },
    ScheduleDelete {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        schedule_id: String,
        #[arg(long)]
        apply: bool,
    },
    ScheduleCreate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    ScheduleUpdate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        schedule_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    SchedulePatch {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        schedule_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    Collections {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "Default")]
        view: String,
    },
    UserGroups {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "Default")]
        _view: String,
    },
    UserGroupDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_group_id: String,
    },
    UserGroupCreate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    UserGroupUpdate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    UserGroupDelete {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        apply: bool,
    },
    UserGroupAddUsers {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    UserGroupRemoveUser {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        apply: bool,
    },
    UserGroupAddAdGroup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    UserGroupRemoveAdGroup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        ad_group_sid: String,
        #[arg(long)]
        apply: bool,
    },
    CollectionDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
    },
    CollectionCreate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        apply: bool,
    },
    CollectionUpdate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CollectionDelete {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        apply: bool,
    },
    CollectionAddUser {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CollectionRemoveUser {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        apply: bool,
    },
    CollectionAddSchedule {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CollectionRemoveSchedule {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        schedule_id: String,
        #[arg(long)]
        apply: bool,
    },
    CollectionAddWorkflow {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CollectionRemoveWorkflow {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        workflow_id: String,
        #[arg(long)]
        apply: bool,
    },
    CollectionAddUserGroup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CollectionRemoveUserGroup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        apply: bool,
    },
    CollectionUpdateUserPermissions {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CollectionUpdateUserGroupPermissions {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        collection_id: String,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    DcmConnections {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
    },
    DcmConnectionLookup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: String,
    },
    DcmConnectionShareCollaboration {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    DcmConnectionShareExecution {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CredentialShareUser {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        credential_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CredentialUnshareUser {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        credential_id: String,
        #[arg(long)]
        user_id: String,
        #[arg(long)]
        apply: bool,
    },
    CredentialShareUserGroup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        credential_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CredentialUnshareUserGroup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        credential_id: String,
        #[arg(long)]
        user_group_id: String,
        #[arg(long)]
        apply: bool,
    },
    Credentials {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        view: Option<String>,
        #[arg(long)]
        user_id: Option<String>,
        #[arg(long)]
        user_group_id: Option<String>,
    },
    CredentialDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        credential_id: String,
    },
    CredentialAdd {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CredentialUpdate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        credential_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    CredentialDelete {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        credential_id: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        apply: bool,
    },
    Subscriptions {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        can_share_schedule: Option<bool>,
        #[arg(long)]
        default_workflow_credential_id: Option<String>,
        #[arg(long)]
        user_count_gte: Option<u32>,
        #[arg(long)]
        user_count_lte: Option<u32>,
        #[arg(long)]
        workflow_count_gte: Option<u32>,
        #[arg(long)]
        workflow_count_lte: Option<u32>,
    },
    SubscriptionDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        subscription_id: String,
    },
    SubscriptionCreate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    SubscriptionUpdate {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        subscription_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    SubscriptionDelete {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        subscription_id: String,
        #[arg(long)]
        apply: bool,
    },
    SubscriptionChangeUsers {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        subscription_id: String,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    DcmAdminConnections {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: Option<String>,
        #[arg(long)]
        visible_by: Option<String>,
    },
    DcmAdminConnectionDetail {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: String,
    },
    DcmAdminConnectionUpsert {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        payload_file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    DcmAdminConnectionDelete {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: String,
        #[arg(long)]
        apply: bool,
    },
    DcmAdminConnectionRemoveCollaboration {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: String,
        #[arg(long)]
        apply: bool,
    },
    DcmAdminConnectionRemoveExecution {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        connection_id: String,
        #[arg(long)]
        apply: bool,
    },
    TransferWorkflowOwner {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        workflow_id: String,
        #[arg(long)]
        owner_id: String,
        #[arg(long, default_value_t = true)]
        transfer_schedules: bool,
        #[arg(long)]
        apply: bool,
        #[arg(long, default_value = "audits")]
        audit_dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum ServerCommand {
    Api {
        #[command(subcommand)]
        command: ServerApiCommand,
    },
    SystemInfo {
        #[arg(long, default_value = "system_info.json")]
        output: PathBuf,
    },
    RuntimeSettings {
        #[arg(long, default_value = DEFAULT_RUNTIME_SETTINGS_PATH)]
        path: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    AyxPaths,
    ServerLogs {
        #[command(subcommand)]
        command: ServerLogsCommand,
    },
    BackupPlan {
        #[arg(long)]
        backup_dir: PathBuf,
    },
    Backup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "backups")]
        backup_dir: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long, default_value = "audits")]
        audit_dir: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum SqlserverCommand {
    Status,
    Inventory,
}

#[derive(Subcommand, Debug)]
enum WorkflowCommand {
    Status,
    Inventory,
    Logs,
}

#[derive(Subcommand, Debug)]
enum CloudCommand {
    Status,
    Inventory,
}

#[derive(Subcommand, Debug)]
enum ServerLogsCommand {
    Discover {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
    },
    Inventory {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
    },
    Summary {
        #[arg(long)]
        path: PathBuf,
    },
    Context {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        needle: String,
        #[arg(long, default_value_t = 25)]
        before: usize,
        #[arg(long, default_value_t = 25)]
        after: usize,
    },
    ParseCsv {
        #[arg(long)]
        path: PathBuf,
    },
    ServiceEvents {
        #[arg(long)]
        path: PathBuf,
    },
    GalleryEvents {
        #[arg(long)]
        path: PathBuf,
    },
    Tail {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value_t = 100)]
        lines: usize,
    },
    Recent {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value_t = 7)]
        days: i64,
    },
}

#[derive(Subcommand, Debug)]
enum ServerApiCommand {
    ImportSwagger {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long, default_value = "3")]
        version: String,
        #[arg(long)]
        url: String,
        #[arg(long, default_value = ".omni/swagger")]
        cache_dir: PathBuf,
    },
    Call {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        operation_id: String,
        #[arg(long, default_value = "3")]
        version: String,
        #[arg(long, default_value = ".omni/swagger")]
        cache_dir: PathBuf,
        #[arg(long)]
        swagger: Option<PathBuf>,
        #[arg(long)]
        body: Option<PathBuf>,
        #[arg(long, value_name = "KEY=VALUE")]
        param: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
enum UpgradeCommand {
    Path {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "embedded-mongo")]
        deployment: String,
    },
    Precheck {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long, default_value = "upgrade-precheck")]
        out: PathBuf,
        #[arg(long, default_value = "embedded-mongo")]
        deployment: String,
    },
    Backup {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        r#type: String,
        #[arg(long, default_value = "upgrade-backup")]
        out: PathBuf,
    },
    Plan {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "upgrade-plan")]
        out: PathBuf,
        #[arg(long, default_value = "embedded-mongo")]
        deployment: String,
    },
    Apply {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        yes: bool,
    },
    Postcheck {
        #[arg(long, default_value = "config.yaml")]
        profile: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long, default_value = "upgrade-postcheck")]
        out: PathBuf,
    },
    Bundle {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

fn load_profile(path: &Path) -> Result<Config> {
    Ok(Config::load_from_path(path)?)
}

fn load_payload(path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read payload file '{}'", path.display()))?;
    let value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse JSON payload from '{}'", path.display()))?;
    Ok(value)
}

fn server_profile(config: &Config) -> Result<&ServerProfile> {
    config.server.as_ref().ok_or_else(|| {
        anyhow!(
            "config missing server section; add server.webapi_url, curator_api_key, and curator_api_secret"
        )
    })
}

fn parse_key_value_params(items: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for item in items {
        let mut parts = item.splitn(2, '=');
        let key = parts
            .next()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| anyhow!("invalid --param '{}', expected key=value", item))?;
        let value = parts
            .next()
            .ok_or_else(|| anyhow!("invalid --param '{}', expected key=value", item))?;
        map.insert(key.to_string(), value.to_string());
    }
    Ok(map)
}

fn execute(cli: Cli) -> Result<Envelope> {
    let envelope = match cli.command {
        Command::Mongo { command } => match command {
            MongoCommand::Status { profile } => {
                let profile = load_profile(&profile)?;
                status_envelope(&profile)?
            }
            MongoCommand::Inventory { profile } => {
                let profile = load_profile(&profile)?;
                inventory_envelope(&profile)?
            }
            MongoCommand::Backup {
                profile,
                output_dir,
                apply,
                audit_dir,
            } => {
                let profile = load_profile(&profile)?;
                backup_envelope(&profile, &output_dir, apply, &audit_dir)?
            }
            MongoCommand::Restore {
                profile,
                input_path,
                apply,
                audit_dir,
            } => {
                let profile = load_profile(&profile)?;
                restore_envelope(&profile, &input_path, apply, &audit_dir)?
            }
        },
        Command::Api { command } => match command {
            ApiCommand::Status { profile } => {
                let profile = load_profile(&profile)?;
                ayx_api::status_envelope(&profile)?
            }
            ApiCommand::Users { profile, view } => {
                let profile = load_profile(&profile)?;
                ayx_api::users_list_envelope(&profile, &view)?
            }
            ApiCommand::UserDetail { profile, user_id } => {
                let profile = load_profile(&profile)?;
                ayx_api::user_detail_envelope(&profile, &user_id)?
            }
            ApiCommand::UserUpdate {
                profile,
                user_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::user_update_envelope(&profile, &user_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "user_id": user_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update the user"
                    });
                    Envelope::ok_with_data("dry-run only: pass --apply to update a user", detail)
                }
            }
            ApiCommand::UserDelete {
                profile,
                user_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::user_delete_envelope(&profile, &user_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "user_id": user_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to delete the user"
                    });
                    Envelope::ok_with_data("dry-run only: pass --apply to delete a user", detail)
                }
            }
            ApiCommand::UserAssets {
                profile,
                user_id,
                asset_type,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::user_assets_envelope(&profile, &user_id, asset_type.as_deref())?
            }
            ApiCommand::UserTransferAssets {
                profile,
                user_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::user_transfer_assets_envelope(&profile, &user_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "user_id": user_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to transfer the user's assets"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to transfer a user's assets",
                        detail,
                    )
                }
            }
            ApiCommand::UserDeactivate {
                profile,
                user_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::user_deactivate_envelope(&profile, &user_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "user_id": user_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to deactivate the user"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to deactivate a user",
                        detail,
                    )
                }
            }
            ApiCommand::UserPasswordReset {
                profile,
                user_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::user_password_reset_envelope(&profile, &user_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "user_id": user_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to send a password reset"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to send a password reset",
                        detail,
                    )
                }
            }
            ApiCommand::Workflows { profile, view } => {
                let profile = load_profile(&profile)?;
                ayx_api::workflows_list_envelope(&profile, &view)?
            }
            ApiCommand::WorkflowDetail {
                profile,
                workflow_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::workflow_detail_envelope(&profile, &workflow_id)?
            }
            ApiCommand::WorkflowJobs {
                profile,
                workflow_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::workflow_jobs_envelope(&profile, &workflow_id)?
            }
            ApiCommand::WorkflowQuestions {
                profile,
                workflow_id,
                version_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::workflow_questions_envelope(&profile, &workflow_id, version_id.as_deref())?
            }
            ApiCommand::WorkflowPackage {
                profile,
                workflow_id,
                version_id,
                output_path,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::workflow_package_envelope(
                    &profile,
                    &workflow_id,
                    version_id.as_deref(),
                    output_path.as_deref(),
                )?
            }
            ApiCommand::WorkflowVersionUpload {
                profile,
                workflow_id,
                name,
                owner_id,
                file_path,
                others_may_download,
                others_can_execute,
                execution_mode,
                has_private_data_exemption,
                comments,
                make_published,
                workflow_credential_type,
                credential_id,
                bypass_workflow_version_check,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::workflow_version_upload_envelope(
                        &profile,
                        &workflow_id,
                        &name,
                        &owner_id,
                        &file_path,
                        others_may_download,
                        others_can_execute,
                        &execution_mode,
                        has_private_data_exemption,
                        comments.as_deref(),
                        make_published,
                        &workflow_credential_type,
                        credential_id.as_deref(),
                        bypass_workflow_version_check,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "workflow_id": workflow_id,
                        "payload_file": file_path.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to upload a workflow version"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to upload a workflow version",
                        detail,
                    )
                }
            }
            ApiCommand::Schedules { profile, view } => {
                let profile = load_profile(&profile)?;
                ayx_api::schedules_list_envelope(&profile, &view)?
            }
            ApiCommand::ScheduleDetail {
                profile,
                schedule_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::schedule_detail_envelope(&profile, &schedule_id)?
            }
            ApiCommand::ScheduleDelete {
                profile,
                schedule_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::schedule_delete_envelope(&profile, &schedule_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "schedule_id": schedule_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to delete the schedule",
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to delete a schedule",
                        detail,
                    )
                }
            }
            ApiCommand::ScheduleCreate {
                profile,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::schedule_create_envelope(&profile, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to create the schedule"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to create a schedule",
                        detail,
                    )
                }
            }
            ApiCommand::ScheduleUpdate {
                profile,
                schedule_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::schedule_update_envelope(&profile, &schedule_id, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "schedule_id": schedule_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update the schedule"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to update a schedule",
                        detail,
                    )
                }
            }
            ApiCommand::SchedulePatch {
                profile,
                schedule_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::schedule_patch_envelope(&profile, &schedule_id, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "schedule_id": schedule_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to patch the schedule"
                    });
                    Envelope::ok_with_data("dry-run only: pass --apply to patch a schedule", detail)
                }
            }
            ApiCommand::Collections { profile, view } => {
                let profile = load_profile(&profile)?;
                ayx_api::collections_list_envelope(&profile, &view)?
            }
            ApiCommand::CollectionDetail {
                profile,
                collection_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::collection_detail_envelope(&profile, &collection_id)?
            }
            ApiCommand::CollectionCreate {
                profile,
                name,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::collection_create_envelope(&profile, &name)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "name": name,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to create the collection",
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to create a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionUpdate {
                profile,
                collection_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::collection_update_envelope(&profile, &collection_id, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to update a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionDelete {
                profile,
                collection_id,
                force,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::collection_delete_envelope(&profile, &collection_id, force)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "force": force,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to delete the collection",
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to delete a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionAddUser {
                profile,
                collection_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::collection_add_user_envelope(&profile, &collection_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to add a user to the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to add a user to a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionRemoveUser {
                profile,
                collection_id,
                user_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::collection_remove_user_envelope(&profile, &collection_id, &user_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "user_id": user_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove the user from the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove a user from a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionAddSchedule {
                profile,
                collection_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::collection_add_schedule_envelope(&profile, &collection_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to add a schedule to the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to add a schedule to a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionRemoveSchedule {
                profile,
                collection_id,
                schedule_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::collection_remove_schedule_envelope(
                        &profile,
                        &collection_id,
                        &schedule_id,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "schedule_id": schedule_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove the schedule from the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove a schedule from a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionAddWorkflow {
                profile,
                collection_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::collection_add_workflow_envelope(&profile, &collection_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to add a workflow to the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to add a workflow to a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionRemoveWorkflow {
                profile,
                collection_id,
                workflow_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::collection_remove_workflow_envelope(
                        &profile,
                        &collection_id,
                        &workflow_id,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "workflow_id": workflow_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove the workflow from the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove a workflow from a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionAddUserGroup {
                profile,
                collection_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::collection_add_user_group_envelope(&profile, &collection_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to add a user group to the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to add a user group to a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionRemoveUserGroup {
                profile,
                collection_id,
                user_group_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::collection_remove_user_group_envelope(
                        &profile,
                        &collection_id,
                        &user_group_id,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "user_group_id": user_group_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove a user group from the collection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove a user group from a collection",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionUpdateUserPermissions {
                profile,
                collection_id,
                user_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::collection_update_user_permissions_envelope(
                        &profile,
                        &collection_id,
                        &user_id,
                        payload,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "user_id": user_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update collection permissions for the user"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to update a user's collection permissions",
                        detail,
                    )
                }
            }
            ApiCommand::CollectionUpdateUserGroupPermissions {
                profile,
                collection_id,
                user_group_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::collection_update_user_group_permissions_envelope(
                        &profile,
                        &collection_id,
                        &user_group_id,
                        payload,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "collection_id": collection_id,
                        "user_group_id": user_group_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update collection permissions for the group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to update a user group's collection permissions",
                        detail,
                    )
                }
            }
            ApiCommand::DcmConnections { profile } => {
                let profile = load_profile(&profile)?;
                ayx_api::dcm_connections_list_envelope(&profile)?
            }
            ApiCommand::DcmConnectionLookup {
                profile,
                connection_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::dcm_connection_lookup_envelope(&profile, &connection_id)?
            }
            ApiCommand::DcmConnectionShareCollaboration {
                profile,
                connection_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::dcm_connection_share_collaboration_envelope(
                        &profile,
                        &connection_id,
                        payload,
                    )?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "connection_id": connection_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to share collaboration access"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to share collaboration access",
                        detail,
                    )
                }
            }
            ApiCommand::DcmConnectionShareExecution {
                profile,
                connection_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::dcm_connection_share_execution_envelope(
                        &profile,
                        &connection_id,
                        payload,
                    )?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "connection_id": connection_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to share execution access"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to share execution access",
                        detail,
                    )
                }
            }
            ApiCommand::UserGroups { profile, .. } => {
                let profile = load_profile(&profile)?;
                ayx_api::user_groups_list_envelope(&profile)?
            }
            ApiCommand::UserGroupDetail {
                profile,
                user_group_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::user_group_detail_envelope(&profile, &user_group_id)?
            }
            ApiCommand::UserGroupCreate {
                profile,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::user_group_create_envelope(&profile, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to create the user group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to create a user group",
                        detail,
                    )
                }
            }
            ApiCommand::UserGroupUpdate {
                profile,
                user_group_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::user_group_update_envelope(&profile, &user_group_id, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "user_group_id": user_group_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update the user group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to update a user group",
                        detail,
                    )
                }
            }
            ApiCommand::UserGroupDelete {
                profile,
                user_group_id,
                force,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::user_group_delete_envelope(&profile, &user_group_id, force)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "user_group_id": user_group_id,
                        "force": force,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to delete the user group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to delete a user group",
                        detail,
                    )
                }
            }
            ApiCommand::UserGroupAddUsers {
                profile,
                user_group_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::user_group_add_users_envelope(&profile, &user_group_id, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "user_group_id": user_group_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to add users to the group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to add users to a user group",
                        detail,
                    )
                }
            }
            ApiCommand::UserGroupRemoveUser {
                profile,
                user_group_id,
                user_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::user_group_remove_user_envelope(&profile, &user_group_id, &user_id)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "user_group_id": user_group_id,
                        "user_id": user_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove a user from the group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove a user from a group",
                        detail,
                    )
                }
            }
            ApiCommand::UserGroupAddAdGroup {
                profile,
                user_group_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::user_group_add_ad_group_envelope(&profile, &user_group_id, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "user_group_id": user_group_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to add an AD group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to add an AD group to a user group",
                        detail,
                    )
                }
            }
            ApiCommand::UserGroupRemoveAdGroup {
                profile,
                user_group_id,
                ad_group_sid,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::user_group_remove_ad_group_envelope(
                        &profile,
                        &user_group_id,
                        &ad_group_sid,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "user_group_id": user_group_id,
                        "ad_group_sid": ad_group_sid,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove the AD group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove an AD group from a user group",
                        detail,
                    )
                }
            }
            ApiCommand::CredentialShareUser {
                profile,
                credential_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::credential_share_user_envelope(&profile, &credential_id, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "credential_id": credential_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to share the credential with a user"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to share a credential with a user",
                        detail,
                    )
                }
            }
            ApiCommand::CredentialUnshareUser {
                profile,
                credential_id,
                user_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::credential_unshare_user_envelope(&profile, &credential_id, &user_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "credential_id": credential_id,
                        "user_id": user_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove the user share"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove a credential share from a user",
                        detail,
                    )
                }
            }
            ApiCommand::CredentialShareUserGroup {
                profile,
                credential_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::credential_share_user_group_envelope(
                        &profile,
                        &credential_id,
                        payload,
                    )?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "credential_id": credential_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to share the credential with a user group"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to share a credential with a user group",
                        detail,
                    )
                }
            }
            ApiCommand::CredentialUnshareUserGroup {
                profile,
                credential_id,
                user_group_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::credential_unshare_user_group_envelope(
                        &profile,
                        &credential_id,
                        &user_group_id,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "credential_id": credential_id,
                        "user_group_id": user_group_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove the user group share"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove a credential share from a user group",
                        detail,
                    )
                }
            }
            ApiCommand::Credentials {
                profile,
                view,
                user_id,
                user_group_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::credentials_list_envelope(
                    &profile,
                    view.as_deref(),
                    user_id.as_deref(),
                    user_group_id.as_deref(),
                )?
            }
            ApiCommand::CredentialDetail {
                profile,
                credential_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::credential_detail_envelope(&profile, &credential_id)?
            }
            ApiCommand::CredentialAdd {
                profile,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::credential_add_envelope(&profile, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to add a credential"
                    });
                    Envelope::ok_with_data("dry-run only: pass --apply to add a credential", detail)
                }
            }
            ApiCommand::CredentialUpdate {
                profile,
                credential_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::credential_update_envelope(&profile, &credential_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "credential_id": credential_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update the credential"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to update a credential",
                        detail,
                    )
                }
            }
            ApiCommand::CredentialDelete {
                profile,
                credential_id,
                force,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::credential_delete_envelope(&profile, &credential_id, force)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "credential_id": credential_id,
                        "force": force,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to delete the credential"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to delete a credential",
                        detail,
                    )
                }
            }
            ApiCommand::Subscriptions {
                profile,
                name,
                can_share_schedule,
                default_workflow_credential_id,
                user_count_gte,
                user_count_lte,
                workflow_count_gte,
                workflow_count_lte,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::subscriptions_list_envelope(
                    &profile,
                    name.as_deref(),
                    can_share_schedule,
                    default_workflow_credential_id.as_deref(),
                    user_count_gte,
                    user_count_lte,
                    workflow_count_gte,
                    workflow_count_lte,
                )?
            }
            ApiCommand::SubscriptionDetail {
                profile,
                subscription_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::subscription_detail_envelope(&profile, &subscription_id)?
            }
            ApiCommand::SubscriptionCreate {
                profile,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::subscription_add_envelope(&profile, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to create a subscription"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to create a subscription",
                        detail,
                    )
                }
            }
            ApiCommand::SubscriptionUpdate {
                profile,
                subscription_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::subscription_update_envelope(&profile, &subscription_id, payload)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "subscription_id": subscription_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to update the subscription"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to update a subscription",
                        detail,
                    )
                }
            }
            ApiCommand::SubscriptionDelete {
                profile,
                subscription_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::subscription_delete_envelope(&profile, &subscription_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "subscription_id": subscription_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to delete the subscription"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to delete a subscription",
                        detail,
                    )
                }
            }
            ApiCommand::SubscriptionChangeUsers {
                profile,
                subscription_id,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::subscription_change_users_envelope(
                        &profile,
                        &subscription_id,
                        payload,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "subscription_id": subscription_id,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to change the subscription users"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to change subscription users",
                        detail,
                    )
                }
            }
            ApiCommand::DcmAdminConnections {
                profile,
                connection_id,
                visible_by,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::dcm_admin_connections_list_envelope(
                    &profile,
                    connection_id.as_deref(),
                    visible_by.as_deref(),
                )?
            }
            ApiCommand::DcmAdminConnectionDetail {
                profile,
                connection_id,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::dcm_admin_connection_detail_envelope(&profile, &connection_id)?
            }
            ApiCommand::DcmAdminConnectionUpsert {
                profile,
                payload_file,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    let payload = load_payload(&payload_file)?;
                    ayx_api::dcm_admin_connection_upsert_envelope(&profile, payload)?
                } else {
                    let detail = json!( {
                        "profile": profile.profile_name,
                        "payload_file": payload_file.display().to_string(),
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to upsert the dcm connection"
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to upsert a dcm connection",
                        detail,
                    )
                }
            }
            ApiCommand::DcmAdminConnectionDelete {
                profile,
                connection_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::dcm_admin_connection_delete_envelope(&profile, &connection_id)?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "connection_id": connection_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to delete the dcm connection",
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to delete a dcm connection",
                        detail,
                    )
                }
            }
            ApiCommand::DcmAdminConnectionRemoveCollaboration {
                profile,
                connection_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::dcm_admin_connection_remove_collaboration_envelope(
                        &profile,
                        &connection_id,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "connection_id": connection_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove collaboration sharing",
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove collaboration sharing",
                        detail,
                    )
                }
            }
            ApiCommand::DcmAdminConnectionRemoveExecution {
                profile,
                connection_id,
                apply,
            } => {
                let profile = load_profile(&profile)?;
                if apply {
                    ayx_api::dcm_admin_connection_remove_execution_envelope(
                        &profile,
                        &connection_id,
                    )?
                } else {
                    let detail = json!({
                        "profile": profile.profile_name,
                        "connection_id": connection_id,
                        "dry_run": true,
                        "applied": false,
                        "suggestion": "pass --apply to remove execution sharing",
                    });
                    Envelope::ok_with_data(
                        "dry-run only: pass --apply to remove execution sharing",
                        detail,
                    )
                }
            }
            ApiCommand::TransferWorkflowOwner {
                profile,
                workflow_id,
                owner_id,
                transfer_schedules,
                apply,
                audit_dir,
            } => {
                let profile = load_profile(&profile)?;
                ayx_api::workflow_transfer_owner_envelope(
                    &profile,
                    &workflow_id,
                    &owner_id,
                    transfer_schedules,
                    apply,
                    &audit_dir,
                )?
            }
        },
        Command::Server { command } => match command {
            None => Envelope::ok("server commands: api, system-info, runtime-settings, ayx-paths, server-logs, backup-plan, backup"),
            Some(ServerCommand::Api { command }) => match command {
                ServerApiCommand::ImportSwagger {
                    profile,
                    version,
                    url,
                    cache_dir,
                } => {
                    let config = load_profile(&profile)?;
                    let server = server_profile(&config)?;
                    let cache_name = format!("{}_swagger_v{}.json", config.profile_name, version);
                    import_swagger(server, &url, &cache_dir, &cache_name)?
                }
                ServerApiCommand::Call {
                    profile,
                    operation_id,
                    version,
                    cache_dir,
                    swagger,
                    body,
                    param,
                } => {
                    let config = load_profile(&profile)?;
                    let server = server_profile(&config)?;
                    let cache_name = format!("{}_swagger_v{}.json", config.profile_name, version);
                    let swagger_path = swagger
                        .clone()
                        .unwrap_or_else(|| cache_dir.join(&cache_name));
                    if !swagger_path.exists() {
                        bail!(
                            "swagger '{}' not found; run server api import-swagger first",
                            swagger_path.display()
                        );
                    }
                    let params = parse_key_value_params(&param)?;
                    let payload = match body {
                        Some(path) => Some(load_payload(&path)?),
                        None => None,
                    };
                    call_operation(server, &operation_id, &params, payload, &swagger_path)?
                }
            },
            Some(ServerCommand::SystemInfo { output }) => {
                let system_info = capture_system_info()?;
                fs::write(&output, serde_json::to_string_pretty(&system_info)?)
                    .with_context(|| format!("failed to write '{}'", output.display()))?;
                Envelope::ok_with_data(
                    "system info captured",
                    json!({ "output": output.display().to_string(), "data": system_info }),
                )
            }
            Some(ServerCommand::RuntimeSettings { path, output }) => {
                let summary = runtime_settings_summary(&path)?;
                if let Some(ref output_path) = output {
                    write_runtime_settings_json(&path, output_path)?;
                }
                Envelope::ok_with_data(
                    "runtime settings summarized",
                    json!({
                        "path": path.display().to_string(),
                        "output": output.as_ref().map(|p| p.display().to_string()),
                        "data": summary
                    }),
                )
            }
            Some(ServerCommand::AyxPaths) => {
                let paths = ayx_paths();
                Envelope::ok_with_data("ayx paths resolved", paths)
            }
            Some(ServerCommand::ServerLogs { command }) => match command {
                ServerLogsCommand::Discover { profile } => {
                    let config = load_profile(&profile)?;
                    Envelope::ok_with_data(
                        "log sources discovered",
                        discover_log_inventory(&config),
                    )
                }
                ServerLogsCommand::Inventory { profile } => {
                    let config = load_profile(&profile)?;
                    Envelope::ok_with_data(
                        "log inventory discovered",
                        discover_log_inventory(&config),
                    )
                }
                ServerLogsCommand::Summary { path } => {
                    let summary = summarize_log_file(&path)?;
                    Envelope::ok_with_data("log summary generated", summary)
                }
                ServerLogsCommand::Context {
                    path,
                    needle,
                    before,
                    after,
                } => {
                    let context = extract_context(&path, &needle, before, after)?;
                    Envelope::ok_with_data("log context extracted", context)
                }
                ServerLogsCommand::ParseCsv { path } => {
                    let parsed = parse_gallery_csv(&path)?;
                    Envelope::ok_with_data("gallery csv parsed", parsed)
                }
                ServerLogsCommand::ServiceEvents { path } => {
                    let parsed = parse_service_events(&path)?;
                    Envelope::ok_with_data("service log events parsed", parsed)
                }
                ServerLogsCommand::GalleryEvents { path } => {
                    let parsed = parse_gallery_events(&path)?;
                    Envelope::ok_with_data("gallery log events parsed", parsed)
                }
                ServerLogsCommand::Tail { path, lines } => {
                    let tail = tail_log_file(&path, lines)?;
                    Envelope::ok_with_data("log tail generated", tail)
                }
                ServerLogsCommand::Recent { profile, days } => {
                    let config = load_profile(&profile)?;
                    Envelope::ok_with_data(
                        "recent log candidates discovered",
                        recent_log_candidates(&config, days),
                    )
                }
            },
            Some(ServerCommand::BackupPlan { backup_dir }) => {
                let plan = backup_plan(&backup_dir)?;
                Envelope::ok_with_data("backup plan generated", plan)
            }
            Some(ServerCommand::Backup {
                profile,
                backup_dir,
                apply,
                audit_dir,
            }) => {
                let config = load_profile(&profile)?;
                let data = run_server_backup(&config, &backup_dir, apply, &audit_dir)?;
                Envelope::ok_with_data(
                    if apply {
                        "server backup executed"
                    } else {
                        "dry-run only: pass --apply to execute server backup"
                    },
                    data,
                )
            }
        },
        Command::Upgrade { command } => match command {
            UpgradeCommand::Path {
                from,
                to,
                deployment,
            } => {
                let detail = compute_path(&from, &to, &deployment);
                Envelope::ok_with_data("upgrade path computed", detail)
            }
            UpgradeCommand::Precheck {
                profile,
                target,
                out,
                deployment,
            } => {
                let config = load_profile(&profile)?;
                let detail = run_precheck(&config, &target, &out, &deployment)?;
                Envelope::ok_with_data("upgrade precheck completed", detail)
            }
            UpgradeCommand::Backup {
                profile,
                r#type,
                out,
            } => {
                let config = load_profile(&profile)?;
                let detail = run_backup(&config, &r#type, &out)?;
                Envelope::ok_with_data("upgrade backup completed", detail)
            }
            UpgradeCommand::Plan {
                from,
                to,
                out,
                deployment,
            } => {
                let detail = run_plan(&from, &to, &deployment, &out)?;
                Envelope::ok_with_data("upgrade plan generated", detail)
            }
            UpgradeCommand::Apply {
                manifest,
                apply,
                yes,
            } => {
                let detail = run_apply(&manifest, apply, yes)?;
                Envelope::ok_with_data("upgrade apply simulated", detail)
            }
            UpgradeCommand::Postcheck {
                profile,
                manifest,
                out,
            } => {
                let config = load_profile(&profile)?;
                let detail = run_postcheck(&config, &manifest, &out)?;
                Envelope::ok_with_data("upgrade postcheck completed", detail)
            }
            UpgradeCommand::Bundle { input, out } => {
                let detail = run_bundle(&input, &out)?;
                Envelope::ok_with_data("upgrade bundle created", detail)
            }
        },
        Command::Sqlserver { command } => match command {
            None => Envelope::ok("sqlserver commands are not yet implemented"),
            Some(SqlserverCommand::Status) => bail!("sqlserver status is not yet implemented"),
            Some(SqlserverCommand::Inventory) => {
                bail!("sqlserver inventory is not yet implemented")
            }
        },
        Command::Workflow { command } => match command {
            None => Envelope::ok("workflow commands are not yet implemented"),
            Some(WorkflowCommand::Status) => bail!("workflow status is not yet implemented"),
            Some(WorkflowCommand::Inventory) => {
                bail!("workflow inventory is not yet implemented")
            }
            Some(WorkflowCommand::Logs) => bail!("workflow logs are not yet implemented"),
        },
        Command::Cloud { command } => match command {
            None => Envelope::ok("cloud commands are not yet implemented"),
            Some(CloudCommand::Status) => bail!("cloud status is not yet implemented"),
            Some(CloudCommand::Inventory) => bail!("cloud inventory is not yet implemented"),
        },
        Command::Update {
            repo_owner,
            repo_name,
            bin_name,
            target_version,
            skip_confirm,
        } => perform_self_update(
            &repo_owner,
            &repo_name,
            &bin_name,
            target_version.as_deref(),
            skip_confirm,
        )?,
    };
    Ok(envelope)
}

fn perform_self_update(
    repo_owner: &str,
    repo_name: &str,
    bin_name: &str,
    target_version: Option<&str>,
    skip_confirm: bool,
) -> Result<Envelope> {
    let target = self_update::get_target();
    let mut builder = GitHubUpdate::configure();
    builder
        .repo_owner(repo_owner)
        .repo_name(repo_name)
        .bin_name(bin_name)
        .current_version(env!("CARGO_PKG_VERSION"))
        .target(target);

    if let Some(version) = target_version {
        builder.target_version_tag(version);
    }
    if skip_confirm {
        builder.no_confirm(true);
    }

    let status = builder.build()?.update()?;
    let detail = match status {
        Status::Updated(version) => json!({
            "result": "updated",
            "version": version,
        }),
        Status::UpToDate(version) => json!({
            "result": "up_to_date",
            "version": version,
        }),
    };

    Ok(Envelope::ok_with_data("self-update complete", detail))
}

fn main() -> Result<()> {
    if wants_help() {
        print_help();
        return Ok(());
    }
    let cli = Cli::parse();
    let output_json = cli.output == "json";

    match execute(cli) {
        Ok(envelope) => {
            if output_json {
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            } else {
                println!("{}", envelope.message);
            }
            Ok(())
        }
        Err(err) => {
            let err_env = Envelope::err_with_data(
                "command failed",
                json!({
                    "error": err.to_string()
                }),
            );
            if output_json {
                println!("{}", serde_json::to_string_pretty(&err_env)?);
            } else {
                eprintln!("{}", err_env.message);
                eprintln!("{}", err);
            }
            Err(err)
        }
    }
}

fn print_help() {
    println!(
        "AYX Rust CLI\n\nUSAGE:\n    ayx [OPTIONS] <COMMAND>\n\nOPTIONS:\n    --help       Print this help message\n    --output     Output format: text or json\n\nCOMMANDS:\n    mongo        Mongo inventory, backup, and restore\n    api          Server API operations\n    server       Server discovery, logs, Swagger, and low-level API calls\n    upgrade      Upgrade planning and execution helpers\n    sqlserver    SQL Server command family (stubbed)\n    workflow     Workflow command family (stubbed)\n    cloud        Cloud command family (stubbed)\n    update       Self-update from GitHub releases\n"
    );
}

fn wants_help() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
}
