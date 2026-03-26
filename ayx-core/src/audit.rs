use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("failed to create audit directory '{path}': {source}")]
    CreateDir {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize audit payload: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to write audit artifact '{path}': {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

pub fn write_audit_artifact(
    audit_dir: &Path,
    operation_prefix: &str,
    payload: &Value,
) -> Result<PathBuf, AuditError> {
    fs::create_dir_all(audit_dir).map_err(|source| AuditError::CreateDir {
        path: audit_dir.display().to_string(),
        source,
    })?;

    let op_id = format!(
        "{}-{}",
        operation_prefix,
        Utc::now().format("%Y%m%dT%H%M%S%.3fZ")
    );
    let artifact_path = audit_dir.join(format!("{}.json", op_id));
    let content = serde_json::to_string_pretty(payload)?;

    fs::write(&artifact_path, content).map_err(|source| AuditError::Write {
        path: artifact_path.display().to_string(),
        source,
    })?;

    Ok(artifact_path)
}
