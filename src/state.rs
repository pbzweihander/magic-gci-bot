//! airspace state management

use std::sync::Arc;

use stopper::Stopper;
use tacview_realtime_client::acmi::RealTimeReader;
use tokio::{io::BufStream, net::TcpStream, sync::RwLock};

#[derive(Debug)]
pub struct AirspaceState {}

impl AirspaceState {
    pub fn new() -> Self {
        Self {}
    }
}

pub async fn state_loop(
    mut tacview_reader: RealTimeReader<BufStream<TcpStream>>,
    _state: Arc<RwLock<AirspaceState>>,
    stopper: Stopper,
) {
    loop {
        match stopper.stop_future(tacview_reader.next()).await {
            Some(Ok(_record)) => {
                // TODO: implement
            }
            Some(Err(error)) => {
                tracing::error!(%error, "Tacview realtime telemetry client read error");
            }
            None => break,
        }
    }
}
