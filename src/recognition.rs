//! recognizing SRS speech to text

use std::{io::Cursor, time::Duration};

use futures_util::{stream::SplitStream, StreamExt};
use srs::VoiceStream;
use stopper::Stopper;

use crate::config::OpenAiConfig;

pub async fn recognition_loop(
    openai_config: OpenAiConfig,
    mut srs_stream: SplitStream<VoiceStream>,
    mut opus_srs_decoder: audiopus::coder::Decoder,
    recognition_tx: tokio::sync::mpsc::UnboundedSender<String>,
    stopper: Stopper,
) {
    'outer: loop {
        let mut buf = Vec::new();

        'inner: loop {
            let res = tokio::time::timeout(
                Duration::from_secs(1),
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
                    tracing::warn!("SRS stream closed. exiting...");
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

        match crate::api::openai::transcribe(&openai_config, voice_buf.into_inner()).await {
            Ok(transcript) => {
                let _ = recognition_tx.send(transcript);
            }
            Err(error) => {
                tracing::error!(%error, "OpenAI transcribe error");
            }
        }
    }
}
