use std::sync::Arc;

use anyhow::Context;
use audiopus::{Channels, SampleRate};
use clap::Parser;
use futures_util::StreamExt;
use stopper::Stopper;
use tokio::sync::RwLock;

use crate::config::{CliConfig, Config};

mod api;
mod config;
mod gci;
mod recognition;
mod state;
mod transmission;

async fn shutdown_signal(stopper: Stopper, stop_tx: tokio::sync::oneshot::Sender<()>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("signal received, starting graceful shutdown");
    let _ = stop_tx.send(());
    stopper.stop();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Get config
    let cli_config = CliConfig::parse();
    tracing::info!("using config file `{}`", cli_config.config.display());
    let config = Config::from_path(&cli_config.config).await?;

    // Init shutdown signal
    let stopper = Stopper::new();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(shutdown_signal(stopper.clone(), stop_tx));

    // Init APIs
    let tacview_reader = crate::api::tacview::connect(&config.tacview).await?;
    let (srs_sink, srs_stream) = crate::api::srs::connect(&config.srs, stop_rx)
        .await?
        .split::<Vec<u8>>();
    let opus_srs_decoder = audiopus::coder::Decoder::new(SampleRate::Hz16000, Channels::Mono)
        .context("failed to initialize Opus decoder")?;

    // Init channels
    let (recognition_tx, recognition_rx) = tokio::sync::mpsc::unbounded_channel();
    let (transmission_tx, transmission_rx) = tokio::sync::mpsc::unbounded_channel();

    // Init state
    let tacview_state = Arc::new(RwLock::new(crate::state::TacviewState::new()));

    // Init main logic loops
    let recognition_handle = tokio::spawn(crate::recognition::recognition_loop(
        config.common.clone(),
        config.openai.clone(),
        tacview_state.clone(),
        srs_stream,
        opus_srs_decoder,
        recognition_tx,
        stopper.clone(),
    ));
    let state_handle = tokio::spawn(crate::state::state_loop(
        tacview_reader,
        tacview_state.clone(),
        stopper.clone(),
    ));
    let gci_handle = tokio::spawn(crate::gci::gci_loop(
        config.common.clone(),
        tacview_state,
        recognition_rx,
        transmission_tx,
        stopper.clone(),
    ));
    let transmission_handle = tokio::spawn(crate::transmission::transmission_loop(
        config.openai.clone(),
        srs_sink,
        transmission_rx,
        stopper,
    ));

    recognition_handle.await?;
    state_handle.await?;
    gci_handle.await?;
    transmission_handle.await?;

    Ok(())
}
