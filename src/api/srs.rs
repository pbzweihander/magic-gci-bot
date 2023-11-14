use std::net::ToSocketAddrs;

use anyhow::Context;

use crate::config::SrsConfig;

pub async fn connect(
    config: &SrsConfig,
    stop_rx: tokio::sync::oneshot::Receiver<()>,
) -> anyhow::Result<srs::VoiceStream> {
    let mut client = srs::Client::new(
        &config.username,
        config.frequency,
        config.coalition.clone().into(),
    );
    client.set_unit(100000001, "External AWACS");

    tracing::info!(
        "connecting to SimpleRadioStandalone server at `{}:{}`",
        config.host,
        config.port
    );

    let (_, game_rx) = futures_channel::mpsc::unbounded();
    let stream = client
        .start(
            (config.host.as_str(), config.port)
                .to_socket_addrs()
                .with_context(|| {
                    format!(
                        "failed to parse host and port `{}:{}`",
                        config.host, config.port
                    )
                })?
                .next()
                .with_context(|| {
                    format!(
                        "failed to parse host and port `{}:{}`",
                        config.host, config.port
                    )
                })?,
            Some(game_rx),
            stop_rx,
        )
        .await
        .with_context(|| {
            format!(
                "failed to connect to SimpleRadioStandalone server at `{}:{}`",
                config.host, config.port
            )
        })?;

    Ok(stream)
}
