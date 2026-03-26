use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("failed to read config file '{path}': {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config yaml '{path}': {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("invalid config: {0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub profile_name: String,
    pub mongo: MongoProfile,
    pub api: Option<ApiProfile>,
    pub alteryx_one: Option<AlteryxOneProfile>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MongoProfile {
    pub mode: MongoMode,
    pub databases: MongoDatabases,
    pub embedded: Option<MongoEmbedded>,
    pub managed: Option<MongoManaged>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MongoDatabases {
    pub gallery_name: String,
    pub service_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MongoMode {
    Embedded,
    Managed,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MongoEmbedded {
    pub runtime_settings_path: Option<String>,
    pub alteryx_service_path: Option<String>,
    pub restore_target_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MongoManaged {
    pub url: Option<String>,
    pub host: Option<String>,
    pub port: u16,
    pub auth_database: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tls: TlsConfig,
    pub timeout_ms: Option<u64>,
    pub retry_count: Option<u32>,
    pub max_pool_size: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TlsConfig {
    pub enabled: bool,
    pub ca_path: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub allow_invalid_hostnames: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiProfile {
    pub base_url: String,
    pub auth: ApiAuth,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ApiAuth {
    pub mode: ApiAuthMode,
    pub pat: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiAuthMode {
    Pat,
    Oauth2ClientCredentials,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AlteryxOneProfile {
    pub account_email: String,
}

impl Config {
    pub fn load_from_path(path: &Path) -> Result<Self, ProfileError> {
        let path_str = path.display().to_string();
        let content = fs::read_to_string(path).map_err(|source| ProfileError::Read {
            path: path_str.clone(),
            source,
        })?;

        let config: Self =
            serde_yaml::from_str(&content).map_err(|source| ProfileError::Parse {
                path: path_str,
                source,
            })?;

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ProfileError> {
        if self.profile_name.trim().is_empty() {
            return Err(ProfileError::Invalid(
                "profile_name cannot be empty".to_string(),
            ));
        }

        if self.mongo.databases.gallery_name.trim().is_empty() {
            return Err(ProfileError::Invalid(
                "mongo.databases.gallery_name cannot be empty".to_string(),
            ));
        }

        if self.mongo.databases.service_name.trim().is_empty() {
            return Err(ProfileError::Invalid(
                "mongo.databases.service_name cannot be empty".to_string(),
            ));
        }

        match self.mongo.mode {
            MongoMode::Embedded => {
                self.mongo.embedded.as_ref().ok_or_else(|| {
                    ProfileError::Invalid("mongo.mode=embedded requires mongo.embedded".to_string())
                })?;
            }
            MongoMode::Managed => {
                let managed = self.mongo.managed.as_ref().ok_or_else(|| {
                    ProfileError::Invalid("mongo.mode=managed requires mongo.managed".to_string())
                })?;

                let has_url = managed.url.as_ref().is_some_and(|u| !u.trim().is_empty());
                let has_host = managed.host.as_ref().is_some_and(|h| !h.trim().is_empty());

                if !has_url && !has_host {
                    return Err(ProfileError::Invalid(
                        "mongo.managed requires either url or host".to_string(),
                    ));
                }

                if managed.port == 0 {
                    return Err(ProfileError::Invalid(
                        "mongo.managed.port must be greater than 0".to_string(),
                    ));
                }
            }
        }

        if let Some(api) = &self.api {
            if api.base_url.trim().is_empty() {
                return Err(ProfileError::Invalid(
                    "api.base_url cannot be empty".to_string(),
                ));
            }

            match api.auth.mode {
                ApiAuthMode::Pat => {
                    let has_pat = api.auth.pat.as_ref().is_some_and(|p| !p.trim().is_empty());
                    if !has_pat {
                        return Err(ProfileError::Invalid(
                            "api.auth.mode=pat requires api.auth.pat".to_string(),
                        ));
                    }
                }
                ApiAuthMode::Oauth2ClientCredentials => {
                    let has_client_id = api
                        .auth
                        .client_id
                        .as_ref()
                        .is_some_and(|v| !v.trim().is_empty());
                    let has_client_secret = api
                        .auth
                        .client_secret
                        .as_ref()
                        .is_some_and(|v| !v.trim().is_empty());
                    if !has_client_id || !has_client_secret {
                        return Err(ProfileError::Invalid(
                            "api.auth.mode=oauth2_client_credentials requires client_id and client_secret"
                                .to_string(),
                        ));
                    }
                }
            }
        }

        if let Some(one) = &self.alteryx_one {
            if !one.account_email.contains('@') {
                return Err(ProfileError::Invalid(
                    "alteryx_one.account_email must be a valid email".to_string(),
                ));
            }
        }

        Ok(())
    }
}
