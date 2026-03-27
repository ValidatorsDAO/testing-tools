use crate::proto::geyser::CommitmentLevel;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigToml {
    pub config: Config,
    pub endpoint: Vec<Endpoint>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub transactions: i32,
    pub account: String,
    pub commitment: ArgsCommitment,
    /// ERPC region to filter results by (e.g. "frankfurt", "amsterdam", "tokyo").
    /// If omitted, all slots are included.
    #[serde(default)]
    pub region: Option<String>,
    /// ERPC edge URL for getLeaderSlots API.
    #[serde(default)]
    pub erpc_url: Option<String>,
    /// ERPC API key for authentication.
    #[serde(default)]
    pub erpc_api_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Endpoint {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_token: Option<String>,
    pub kind: EndpointKind,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EndpointKind {
    Yellowstone,
    Arpc,
    Thor,
    Shredstream,
    Shreder,
    Jetstream,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgsCommitment {
    #[default]
    Processed,
    Confirmed,
    Finalized,
}

impl From<ArgsCommitment> for CommitmentLevel {
    fn from(commitment: ArgsCommitment) -> Self {
        match commitment {
            ArgsCommitment::Processed => CommitmentLevel::Processed,
            ArgsCommitment::Confirmed => CommitmentLevel::Confirmed,
            ArgsCommitment::Finalized => CommitmentLevel::Finalized,
        }
    }
}

impl ArgsCommitment {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArgsCommitment::Processed => "processed",
            ArgsCommitment::Confirmed => "confirmed",
            ArgsCommitment::Finalized => "finalized",
        }
    }
}

/// Known ERPC regions for display and validation.
pub const KNOWN_REGIONS: &[&str] = &[
    "amsterdam",
    "frankfurt",
    "ny",
    "chicago",
    "stockholm",
    "saltlakecity",
    "tokyo",
    "london",
    "singapore",
    "dublin",
    "sydney",
    "losangeles",
    "hongkong",
];

impl ConfigToml {
    pub fn load(path: &str) -> Result<Self> {
        let content =
            fs::read_to_string(path).with_context(|| format!("Failed to read config {}", path))?;
        let config = toml::from_str(&content).map_err(|err| anyhow!(err))?;
        Ok(config)
    }

    pub fn create_default(path: &str) -> Result<Self> {
        let default_config = ConfigToml {
            config: Config {
                transactions: 1000,
                account: "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA".to_string(),
                commitment: ArgsCommitment::Processed,
                region: None,
                erpc_url: None,
                erpc_api_key: None,
            },
            endpoint: vec![
                Endpoint {
                    name: "grpc".to_string(),
                    url: "http://127.0.0.1:1001".to_string(),
                    x_token: None,
                    kind: EndpointKind::Yellowstone,
                },
            ],
        };

        let toml_string = toml::to_string_pretty(&default_config)
            .context("Failed to serialize default config")?;
        fs::write(path, toml_string)
            .with_context(|| format!("Failed to write default config {}", path))?;

        Ok(default_config)
    }

    pub fn load_or_create(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            Self::load(path)
        } else {
            Self::create_default(path)
        }
    }
}
