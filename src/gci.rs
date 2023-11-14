//! Module about actual GCIing logic

use std::sync::Arc;

use stopper::Stopper;
use tokio::sync::RwLock;

use crate::state::AirspaceState;

pub async fn gci_loop(
    _state: Arc<RwLock<AirspaceState>>,
    mut recognition_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    _transmission_tx: tokio::sync::mpsc::UnboundedSender<String>,
    stopper: Stopper,
) {
    while let Some(_line) = stopper.stop_future(recognition_rx.recv()).await.flatten() {
        // TODO: implement
    }
}
