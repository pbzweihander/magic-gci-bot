use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::Parser;
use serde::Deserialize;

#[derive(Clone, Parser)]
pub struct CliConfig {
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,
}

#[derive(Clone, Deserialize)]
pub enum Coalition {
    Blue,
    Red,
}

impl Coalition {
    pub fn flip(&self) -> Self {
        match self {
            Self::Blue => Self::Red,
            Self::Red => Self::Blue,
        }
    }

    pub fn as_tacview_coalition(&self) -> &'static str {
        match self {
            Self::Blue => "Enemies",
            Self::Red => "Allies",
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct CommonConfig {
    pub callsign: String,
    pub coalition: Coalition,
}

#[derive(Clone, Deserialize)]
pub struct TacviewConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Clone, Deserialize)]
pub enum SrsConfigCoalition {
    Spectator,
    Blue,
    Red,
}

impl From<SrsConfigCoalition> for srs::message::Coalition {
    fn from(value: SrsConfigCoalition) -> srs::message::Coalition {
        match value {
            SrsConfigCoalition::Spectator => Self::Spectator,
            SrsConfigCoalition::Blue => Self::Blue,
            SrsConfigCoalition::Red => Self::Red,
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct SrsConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub coalition: SrsConfigCoalition,
    pub frequency: u64,
}

#[derive(Clone, Deserialize)]
pub struct OpenAiConfig {
    pub api_key: String,
    pub speech_voice: String,
    pub speech_speed: f64,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub common: CommonConfig,
    pub tacview: TacviewConfig,
    pub srs: SrsConfig,
    pub openai: OpenAiConfig,
}

impl Config {
    pub async fn from_path(path: &Path) -> anyhow::Result<Self> {
        let s = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("failed to read config file `{}`", path.display()))?;
        toml::from_str(&s)
            .with_context(|| format!("failed to parse config file `{}`", path.display()))
    }
}
