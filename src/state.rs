//! airspace state management

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use stopper::Stopper;
use tacview_realtime_client::acmi::{
    record::{
        global_property::GlobalProperty,
        object_property::{Coords, ObjectProperty, Tag},
        Record,
    },
    RealTimeReader,
};
use tokio::{io::BufStream, net::TcpStream, sync::RwLock};

#[derive(Debug, Default)]
pub struct TacviewObject {
    pub coords: Coords,
    pub ty: HashSet<Tag>,
    pub name: Option<String>,
    pub pilot: Option<String>,
    pub coalition: Option<String>,
}

#[derive(Debug, Default)]
pub struct TacviewState {
    pub reference_longitude: Option<f64>,
    pub reference_latitude: Option<f64>,
    pub objects: BTreeMap<u64, TacviewObject>,
}

impl TacviewState {
    pub fn find_air_object_by_callsign(
        &self,
        callsign: &str,
        coalition: &str,
    ) -> Option<&TacviewObject> {
        self.objects.values().find(|object| {
            object.ty.contains(&Tag::Air)
                && object.coalition.as_deref() == Some(coalition)
                && object
                    .pilot
                    .as_ref()
                    .map(|pilot| {
                        pilot
                            .trim()
                            .to_lowercase()
                            .replace(['-', ' '], "")
                            .contains(&callsign.trim().to_lowercase().replace(['-', ' '], ""))
                    })
                    .unwrap_or(false)
        })
    }

    pub fn list_air_object_by_coalition<'a>(
        &'a self,
        coalition: &'a str,
    ) -> impl Iterator<Item = &TacviewObject> + 'a {
        self.objects.values().filter(|object| {
            object.ty.contains(&Tag::Air) && object.coalition.as_deref() == Some(coalition)
        })
    }

    pub fn list_air_callsigns_by_coalition<'a>(
        &'a self,
        coalition: &'a str,
    ) -> impl Iterator<Item = String> + 'a {
        self.objects
            .values()
            .filter(|object| {
                object.ty.contains(&Tag::Air) && object.coalition.as_deref() == Some(coalition)
            })
            .filter_map(|object| object.pilot.clone())
    }
}

impl TacviewState {
    pub fn new() -> Self {
        Self::default()
    }
}

pub async fn state_loop(
    mut tacview_reader: RealTimeReader<BufStream<TcpStream>>,
    state: Arc<RwLock<TacviewState>>,
    stopper: Stopper,
) {
    loop {
        match stopper.stop_future(tacview_reader.next()).await {
            Some(Ok(record)) => match record {
                Record::Remove(id) => {
                    let mut state = state.write().await;
                    state.objects.remove(&id);
                }
                Record::Frame(_) => {
                    // Do nothing
                }
                Record::Event(_) => {
                    // Do nothing
                }
                Record::GlobalProperties(global_properties) => {
                    for global_property in global_properties {
                        match global_property {
                            GlobalProperty::ReferenceLatitude(lat) => {
                                let mut state = state.write().await;
                                state.reference_latitude = Some(lat);

                                // When ReferenceLatitude occured, assume new connection was made, so clear the objects.
                                state.objects.clear();
                            }
                            GlobalProperty::ReferenceLongitude(lng) => {
                                let mut state = state.write().await;
                                state.reference_longitude = Some(lng);

                                // When ReferenceLongitude occured, assume new connection was made, so clear the objects.
                                state.objects.clear();
                            }
                            _ => {}
                        }
                    }
                }
                Record::Update(id, object_properties) => {
                    let mut state = state.write().await;
                    let object = state.objects.entry(id).or_default();
                    for object_property in object_properties {
                        match object_property {
                            ObjectProperty::T(coords) => {
                                object.coords.update(&coords);
                            }
                            ObjectProperty::Type(ty) => {
                                object.ty = ty;
                            }
                            ObjectProperty::Name(name) => {
                                object.name = Some(name);
                            }
                            ObjectProperty::Pilot(pilot) => {
                                object.pilot = Some(pilot);
                            }
                            ObjectProperty::Coalition(coalition) => {
                                object.coalition = Some(coalition);
                            }
                            _ => {}
                        }
                    }
                }
            },
            Some(Err(error)) => {
                tracing::error!(%error, "Tacview realtime telemetry client read error");
            }
            None => break,
        }
    }
    tracing::info!("exiting state loop");
}
