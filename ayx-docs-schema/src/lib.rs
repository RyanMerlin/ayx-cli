use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CliSchema {
    pub version: &'static str,
}
