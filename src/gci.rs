//! Module about actual GCIing logic

use std::sync::Arc;

use geo::{HaversineBearing, Point};
use itertools::Itertools;
use stopper::Stopper;
use tokio::sync::RwLock;

use crate::{
    config::CommonConfig,
    recognition::{IncomingTransmission, Intent},
    state::TacviewState,
    transmission::OutgoingTransmission,
};

fn meters_to_feet(meters: f64) -> f64 {
    meters * 3.28084
}

fn get_bearing((lat1, lon1): (f64, f64), (lat2, lon2): (f64, f64)) -> f64 {
    Point::new(lon1, lat1).haversine_bearing(Point::new(lon2, lat2))
}

/// In nautical miles
fn get_range((lat1, lon1): (f64, f64), (lat2, lon2): (f64, f64)) -> f64 {
    const R: f64 = 6371.;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let d_lat_half_sin = (d_lat / 2.).sin();
    let d_lon_half_sin = (d_lon / 2.).sin();

    let a = d_lat_half_sin * d_lat_half_sin
        + d_lon_half_sin * d_lon_half_sin * lat1_rad.cos() * lat2_rad.cos();
    let c = 2. * a.sqrt().atan2((1. - a).sqrt());
    let d = R * c;
    d * 0.539957
}

fn get_cardinal_point(heading: f64) -> &'static str {
    match (heading as isize + 360) % 360 {
        0..=22 | 338..=360 => "north",
        23..=67 => "north east",
        68..=112 => "east",
        113..=157 => "south east",
        158..=202 => "south",
        203..=247 => "south west",
        248..=292 => "west",
        _ => "north west",
    }
}

fn get_aircraft_ty(name: Option<&str>) -> &str {
    match name {
        Some("Tornado GR4") | Some("Tornado IDS") => "tornado",
        Some("F/A-18A") | Some("F/A-18C") | Some("FA-18C_hornet") => "hornet",
        Some("F-14A") | Some("F-14B") | Some("F-14A-135-GR") => "tomcat",
        Some("Tu-22M3") => "backfire",
        Some("F-4E") => "phantom",
        Some("B-52H") => "stratofortress",
        Some("MiG-23MLD") | Some("MiG-27K") => "flogger",
        Some("Su-27") | Some("Su-30") | Some("Su-33") | Some("J-11A") => "flanker",
        Some("Su-25") | Some("Su-25TM") | Some("Su-25T") => "frogfoot",
        Some("MiG-25PD") | Some("MiG-25RBT") => "foxbat",
        Some("Su-17M4") => "fitter",
        Some("MiG-31") => "foxhound",
        Some("Tu-95MS") | Some("Tu-142") => "bear",
        Some("Su-24M") | Some("Su-24MR") => "fencer",
        Some("Tu-160") => "blackjack",
        Some("F-117A") => "nighthawk",
        Some("B-1B") => "lancer",
        Some("S-3B") | Some("S-3B Tanker") => "viking",
        Some("M-2000C") | Some("Mirage 2000-5") => "mirage",
        Some("F-15C") | Some("F-15E") | Some("F-15ESE") => "eagle",
        Some("MiG-29A") | Some("MiG-29G") | Some("MiG-29S") => "fulcrum",
        Some("C-130") => "hercules",
        Some("An-26B") => "curl",
        Some("An-30M") => "clank",
        Some("C-17A") => "globemaster",
        Some("A-50") => "mainstay",
        Some("E-3A") => "sentry",
        Some("IL-78M") => "midas",
        Some("E-2C") => "hawkeye",
        Some("IL-76MD") => "candid",
        Some("F-16A") | Some("F-16A MLU") | Some("F-16C_50") | Some("F-16C bl.50")
        | Some("F-16C bl.52d") => "viper",
        Some("RQ-1A Predator") => "predator",
        Some("Yak-40") => "codling",
        Some("KC-130") => "hercules tanker",
        Some("KC-135") | Some("KC135MPRS") => "stratotanker",
        Some("A-20G") => "havok",
        Some("A-10A") | Some("A-10C") | Some("A-10C_2") => "warthog",
        Some("AJS37") => "viggen",
        Some("AV8BNA") => "harrier",
        Some("C-101EB") | Some("C-101CC") => "aviojet",
        Some("JF-17") => "thunder",
        Some("KJ-2000") => "mainring",
        Some("WingLoong-I") => "wing loong",
        Some("F-5E") | Some("F-5E-3") => "tiger",
        Some("F-86F Sabre") => "saber",
        Some("Hawk") => "hawk",
        Some("L-39C") | Some("L-39ZA") => "albatros",
        Some("MQ-9 Reaper") => "reaper",
        Some("MiG-15bis") => "fagot",
        Some("MiG-19P") => "farmer",
        Some("MiG-21Bis") => "fishbed",
        Some("Su-34") => "fullback",
        Some("Ka-50") | Some("Ka-50_3") => "black shark",
        Some("Mi-24V") | Some("Mi-24P") => "hind",
        Some("Mi-8MT") => "hip",
        Some("Mi-26") => "halo",
        Some("Ka-27") => "helix",
        Some("UH-60A") => "black hawk",
        Some("CH-53E") => "super stallion",
        Some("CH-47D") => "chinook",
        Some("SH-3W") => "sea king",
        Some("AH-64A") | Some("AH-64D") | Some("AH-64D_BLK_II") => "apache",
        Some("AH-1W") => "cobra",
        Some("SH-60B") => "seahawk",
        Some("UH-1H") => "huey",
        Some("Mi-28N") => "havoc",
        Some("OH-58D") => "kiowa",
        Some("SA342M") | Some("SA342L") | Some("SA342Mistral") | Some("SA342Minigun") => "gazelle",
        Some(name) => name,
        None => "unknown",
    }
}

pub async fn gci_loop(
    common_config: CommonConfig,
    state: Arc<RwLock<TacviewState>>,
    mut recognition_rx: tokio::sync::mpsc::UnboundedReceiver<IncomingTransmission>,
    transmission_tx: tokio::sync::mpsc::UnboundedSender<OutgoingTransmission>,
    stopper: Stopper,
) {
    while let Some(incoming_transmission) =
        stopper.stop_future(recognition_rx.recv()).await.flatten()
    {
        if incoming_transmission.to_callsign.to_lowercase() == common_config.callsign.to_lowercase()
        {
            match incoming_transmission.intent {
                Intent::Unknown => {
                    continue;
                }
                Intent::RadioCheck => {
                    let _ = transmission_tx.send(OutgoingTransmission {
                        to_callsign: incoming_transmission.from_callsign,
                        from_callsign: common_config.callsign.clone(),
                        message: "5 by 5".to_string(),
                    });
                }
                Intent::RequestBogeyDope => {
                    let state = state.read().await;
                    handle_bogey_dope(
                        incoming_transmission,
                        &state,
                        &common_config,
                        &transmission_tx,
                    );
                }
            }
        } else {
            tracing::warn!(to_callsign = %incoming_transmission.to_callsign, "incoming transmission is not for the AWACS");
        }
    }
    tracing::info!("exiting GCI loop");
}

fn handle_bogey_dope(
    incoming_transmission: IncomingTransmission,
    state: &TacviewState,
    common_config: &CommonConfig,
    transmission_tx: &tokio::sync::mpsc::UnboundedSender<OutgoingTransmission>,
) {
    if let Some(from_object) = state.find_air_object_by_callsign(
        &incoming_transmission.from_callsign,
        common_config.coalition.as_tacview_coalition(),
    ) {
        if from_object.coalition.as_deref() == Some(common_config.coalition.as_tacview_coalition())
        {
            if let (
                Some(reference_latitude),
                Some(reference_longitude),
                Some(from_object_latitude),
                Some(from_object_longitude),
            ) = (
                state.reference_latitude,
                state.reference_longitude,
                from_object.coords.latitude,
                from_object.coords.longitude,
            ) {
                let from_object_latlng = (
                    reference_latitude + from_object_latitude,
                    reference_longitude + from_object_longitude,
                );

                let bandits = state.list_air_object_by_coalition(
                    common_config.coalition.flip().as_tacview_coalition(),
                );

                if let Some((closest_bandit, range)) = bandits
                    .filter_map(|bandit| {
                        if let (Some(bandit_lat), Some(bandit_lng), Some(_), Some(_)) = (
                            bandit.coords.latitude,
                            bandit.coords.longitude,
                            bandit.coords.altitude,
                            bandit.coords.heading,
                        ) {
                            let bandit_latlng = (
                                reference_latitude + bandit_lat,
                                reference_longitude + bandit_lng,
                            );
                            Some((bandit, get_range(from_object_latlng, bandit_latlng)))
                        } else {
                            None
                        }
                    })
                    .min_by(|(_bandit1, range1), (_bandit2, range2)| {
                        range1.partial_cmp(range2).unwrap()
                    })
                {
                    let bandit_latlng = (
                        reference_latitude + closest_bandit.coords.latitude.unwrap(),
                        reference_longitude + closest_bandit.coords.longitude.unwrap(),
                    );

                    let bearing = get_bearing(from_object_latlng, bandit_latlng);

                    let range = range as usize;

                    let altitude_thousands =
                        meters_to_feet(closest_bandit.coords.altitude.unwrap()) / 1000.;
                    let altitude_str = match altitude_thousands as usize {
                        0 => "on the deck".to_string(),
                        1 => "one thousand".to_string(),
                        a => format!("{} thousands", a),
                    };

                    let bandit_heading = closest_bandit.coords.heading.unwrap();
                    let aspect_degrees = (((bearing - bandit_heading) as isize) + 360) % 360;
                    let bandit_heading_cardinal = get_cardinal_point(bandit_heading);
                    let aspect = match aspect_degrees {
                        0..=65 | 295..=360 => {
                            format!("dragging {}", bandit_heading_cardinal)
                        }
                        66..=115 | 245..=294 => {
                            format!("beaming {}", bandit_heading_cardinal)
                        }
                        116..=155 | 205..=244 => {
                            format!("flanking {}", bandit_heading_cardinal)
                        }
                        _ => "hot".to_string(),
                    };

                    let bearing = ((bearing as isize) + 360) % 360;
                    let bearing_str = format!("{:03}", bearing).chars().join(" ");

                    let ty = get_aircraft_ty(closest_bandit.name.as_deref());

                    let _ = transmission_tx.send(OutgoingTransmission {
                        to_callsign: incoming_transmission.from_callsign,
                        from_callsign: common_config.callsign.clone(),
                        message: format!("bandit braa {bearing_str}, for {range} miles, {altitude_str}, {aspect}, type {ty}"),
                    });
                } else {
                    let _ = transmission_tx.send(OutgoingTransmission {
                        to_callsign: incoming_transmission.from_callsign,
                        from_callsign: common_config.callsign.clone(),
                        message: "Scope is currently clear".to_string(),
                    });
                }
            } else {
                tracing::warn!("Tacview state is not initialized");
            }
        } else {
            let _ = transmission_tx.send(OutgoingTransmission {
                to_callsign: incoming_transmission.from_callsign,
                from_callsign: common_config.callsign.clone(),
                message: "You are not in my coalition".to_string(),
            });
        }
    } else {
        let _ = transmission_tx.send(OutgoingTransmission {
            to_callsign: incoming_transmission.from_callsign,
            from_callsign: common_config.callsign.clone(),
            message: "I cannot find you on scope".to_string(),
        });
    }
}
