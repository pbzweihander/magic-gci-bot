use anyhow::Context;
use tacview_realtime_client::acmi::RealTimeReader;
use tokio::{io::BufStream, net::TcpStream};

use crate::config::TacviewConfig;

pub async fn connect(
    config: &TacviewConfig,
) -> anyhow::Result<RealTimeReader<BufStream<TcpStream>>> {
    tracing::info!(
        "connecting to Tacview realtime telemetry server at `{}:{}`",
        config.host,
        config.port
    );
    tacview_realtime_client::connect(
        (config.host.as_str(), config.port),
        &config.username,
        &config.password.clone().unwrap_or_default(),
    )
    .await
    .with_context(|| {
        format!(
            "failed to connect to Tacview realtime telemetry server at `{}:{}`",
            config.host, config.port
        )
    })
}
