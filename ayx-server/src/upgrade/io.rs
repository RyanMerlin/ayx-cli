use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use csv::Writer;

pub fn ensure_dir(path: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    Ok(path.to_path_buf())
}

pub fn write_json(path: &Path, payload: &serde_json::Value) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let content = serde_json::to_string_pretty(payload)?;
    fs::write(path, content)?;
    Ok(path.to_path_buf())
}

pub fn write_csv(
    path: &Path,
    rows: &[std::collections::HashMap<String, String>],
    fieldnames: Option<&[&str]>,
) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    let headers: Vec<String> = fieldnames
        .map(|list| list.iter().map(|s| s.to_string()).collect())
        .unwrap_or_else(|| {
            let mut keys: Vec<String> = rows.iter().flat_map(|row| row.keys().cloned()).collect();
            keys.sort();
            keys.dedup();
            keys
        });

    let mut writer = Writer::from_path(path)?;
    writer.write_record(&headers)?;

    for row in rows {
        let record: Vec<&str> = headers
            .iter()
            .map(|key| row.get(key).map(String::as_str).unwrap_or(""))
            .collect();
        writer.write_record(&record)?;
    }
    writer.flush()?;
    Ok(path.to_path_buf())
}

pub fn copy_if_exists(source: &Path, destination: &Path) -> Result<(bool, String)> {
    if !source.exists() {
        return Ok((false, format!("missing:{}", source.display())));
    }
    if let Some(parent) = destination.parent() {
        ensure_dir(parent)?;
    }
    if source.is_dir() {
        fs_extra::dir::copy(
            source,
            destination,
            &fs_extra::dir::CopyOptions {
                overwrite: true,
                copy_inside: true,
                ..Default::default()
            },
        )?;
    } else {
        fs::copy(source, destination)?;
    }
    Ok((true, format!("copied:{}", source.display())))
}
