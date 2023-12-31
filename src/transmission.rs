//! transmitting a sentence to SRS

use std::{
    io::Cursor,
    time::{Duration, Instant},
};

use anyhow::Context;
use futures_util::{stream::SplitSink, SinkExt};
use srs::VoiceStream;
use stopper::Stopper;

use crate::config::OpenAiConfig;

#[derive(Debug)]
pub struct OutgoingTransmission {
    pub to_callsign: String,
    pub from_callsign: String,
    pub message: String,
}

impl OutgoingTransmission {
    fn to_speech_string(&self) -> String {
        format!(
            "{}, {}, {}",
            self.to_callsign, self.from_callsign, self.message
        )
    }
}

pub async fn transmission_loop(
    openai_config: OpenAiConfig,
    mut srs_sink: SplitSink<VoiceStream, Vec<u8>>,
    mut transmission_rx: tokio::sync::mpsc::UnboundedReceiver<OutgoingTransmission>,
    stopper: Stopper,
) {
    while let Some(outgoing_transmission) =
        stopper.stop_future(transmission_rx.recv()).await.flatten()
    {
        tracing::info!(?outgoing_transmission, "outgoing transmission");
        if let Err(error) = transmit(
            outgoing_transmission.to_speech_string(),
            &openai_config,
            &mut srs_sink,
        )
        .await
        {
            tracing::error!(%error, "transmit error");
        }
    }
    tracing::info!("exiting transmission loop");
}

async fn transmit(
    line: String,
    openai_config: &OpenAiConfig,
    srs_sink: &mut SplitSink<VoiceStream, Vec<u8>>,
) -> anyhow::Result<()> {
    let speech_ogg = crate::api::openai::speech(openai_config, &line).await?;
    let mut ogg_reader = ogg::PacketReader::new(Cursor::new(speech_ogg));

    ogg_reader
        .read_packet_expected()
        .context("failed to read from OGG reader")?; // header
    ogg_reader
        .read_packet_expected()
        .context("failed to read from OGG reader")?; // tag

    let mut frames = Vec::new();

    while let Some(packet) = ogg_reader
        .read_packet()
        .context("failed to read from OGG reader")?
    {
        frames.push(packet.data);
    }

    let start = Instant::now();
    for (i, frame) in frames.iter().enumerate() {
        srs_sink
            .send(frame.clone())
            .await
            .context("failed to send to SRS")?;

        let playtime = Duration::from_millis((i as u64 + 1) * 20);
        let elapsed = start.elapsed();
        if playtime > elapsed {
            tokio::time::sleep(playtime - elapsed).await;
        }
    }
    srs_sink
        .flush()
        .await
        .context("failed to flush SRS stream")?;

    Ok(())
}
