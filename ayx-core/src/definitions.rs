use std::path::PathBuf;

pub const GALLERY_APPID: &str = "{eea9431a-a3d4-4c9b-9f9a-b83916c11c67}";
pub const DEFAULT_OUTPUT_DIR: &str = "output";
pub const DEFAULT_CONFIG_PATH: &str = "config.yaml";
pub const DEFAULT_RUNTIME_SETTINGS_PATH: &str = r"C:\ProgramData\Alteryx\RuntimeSettings.xml";
pub const DEFAULT_MONGO_RESTORE_TARGET_PATH: &str =
    r"C:\ProgramData\Alteryx\Service\Persistence\MongoDB";

pub fn default_output_dir() -> PathBuf {
    PathBuf::from(DEFAULT_OUTPUT_DIR)
}
