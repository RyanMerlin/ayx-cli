use std::path::Path;

use crate::profile::{Config, ProfileError};

pub fn load_config(path: &Path) -> Result<Config, ProfileError> {
    Config::load_from_path(path)
}
