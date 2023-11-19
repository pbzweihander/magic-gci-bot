//! recognizing incoming SRS transmission

use std::{io::Cursor, sync::Arc, time::Duration};

use futures_util::{stream::SplitStream, StreamExt};
use serde::Deserialize;
use srs::VoiceStream;
use stopper::Stopper;
use tokio::sync::RwLock;

use crate::{
    config::{CommonConfig, OpenAiConfig},
    state::TacviewState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    RadioCheck,
    RequestBogeyDope,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct IncomingTransmission {
    pub to_callsign: String,
    pub from_callsign: String,
    pub intent: Intent,
}

pub async fn recognition_loop(
    common_config: CommonConfig,
    openai_config: OpenAiConfig,
    state: Arc<RwLock<TacviewState>>,
    mut srs_stream: SplitStream<VoiceStream>,
    mut opus_srs_decoder: audiopus::coder::Decoder,
    recognition_tx: tokio::sync::mpsc::UnboundedSender<IncomingTransmission>,
    stopper: Stopper,
) {
    'outer: loop {
        let mut buf = Vec::new();

        'inner: loop {
            let res = tokio::time::timeout(
                Duration::from_millis(500),
                stopper.stop_future(srs_stream.next()),
            )
            .await;

            match res {
                Ok(Some(Some(Ok(packet)))) => {
                    let mut decode_buf = [0i16; 5760];
                    match opus_srs_decoder.decode(
                        Some(&packet.audio_part),
                        &mut decode_buf[..],
                        false,
                    ) {
                        Ok(len) => buf.extend_from_slice(&decode_buf[0..len]),
                        Err(error) => {
                            tracing::error!(%error, "Opus decoder error");
                        }
                    }
                }
                Ok(Some(Some(Err(error)))) => {
                    tracing::error!(%error, "SRS stream error");
                }
                Ok(None) | Ok(Some(None)) => {
                    break 'outer;
                }
                Err(_) => {
                    break 'inner;
                }
            }
        }

        if buf.is_empty() {
            continue;
        }

        let mut voice_buf = Cursor::new(Vec::new());
        wav::write(
            wav::Header::new(wav::WAV_FORMAT_PCM, 1, 16000, 16),
            &wav::BitDepth::Sixteen(buf),
            &mut voice_buf,
        )
        .unwrap();

        let possible_callsigns = {
            let state = state.read().await;
            state
                .list_air_callsigns_by_coalition(common_config.coalition.as_tacview_coalition())
                .flat_map(|callsign| {
                    callsign
                        .split('|')
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                })
                .map(|callsign| callsign.trim().to_string())
                .collect::<Vec<_>>()
        };
        match crate::api::openai::transcribe(
            &openai_config,
            &common_config.callsign,
            &possible_callsigns,
            voice_buf.into_inner(),
        )
        .await
        {
            Ok(transcript) => {
                if transcript.is_empty() {
                    continue;
                }

                tracing::info!(%transcript, "parsing transcript");
                match crate::api::openai::parse_transmission(
                    &openai_config,
                    &common_config.callsign,
                    transcript.clone(),
                )
                .await
                {
                    Ok(incoming_transmission) => {
                        tracing::info!(?incoming_transmission, "incoming transmission");
                        let _ = recognition_tx.send(incoming_transmission);
                    }
                    Err(error) => {
                        tracing::error!(%transcript, %error, "failed to parse incoming transmission");
                    }
                }
            }
            Err(error) => {
                tracing::error!(%error, "OpenAI transcribe error");
            }
        }
    }
    tracing::info!("exiting recognition loop");
}
