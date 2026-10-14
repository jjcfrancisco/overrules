//! The JSON contract the page (`web/app.js`) relies on, checked against real segments.

use std::collections::HashMap;

use overrules::Network;
use overrules_explorer::api;
use serde_json::Value;

fn net() -> Network {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../core/tests/fixtures/seville.geojson"
    );
    Network::new(overrules::io::read_path(path).expect("fixture"))
}

fn resolve(pairs: &[(&str, &str)]) -> Result<Value, overrules_explorer::Error> {
    let p: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    api::resolve(&net(), &p)
}

#[test]
fn resolve_by_segment_and_position() {
    let v = resolve(&[
        ("segment", "e26d3384"),
        ("at", "0.05"),
        ("heading", "forward"),
        ("mode", "car"),
    ])
    .unwrap();
    assert_eq!(v["speed_limit"]["determining"], 5);
    assert_eq!(v["speed_limit"]["value"]["max_speed"]["value"], 80);
    assert_eq!(v["speed_limit"]["rules"].as_array().unwrap().len(), 8);
    assert_eq!(v["access"]["value"], Value::Null);
    assert!(v["position"]["bearing_forward_deg"].is_f64());
    assert!(
        v["boundaries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["planar_error_m"].is_f64())
    );
    assert_eq!(v["situation"]["mode"], "car");
}

#[test]
fn resolve_by_point_snaps_and_measures_the_planar_error() {
    let v = resolve(&[
        ("lon", "-6.0030"),
        ("lat", "37.3866"),
        ("heading", "forward"),
    ])
    .unwrap();
    assert!(v["position"]["snapped_from_m"].is_f64());
    assert!(v["position"]["planar_error_m"].is_f64());
    assert!(v["nearby"].is_array());
}

#[test]
fn banned_turns_carry_their_connector() {
    let v = resolve(&[
        ("segment", "a022afe5"),
        ("at", "0.5"),
        ("heading", "forward"),
    ])
    .unwrap();
    assert_eq!(
        v["prohibited_transitions"]["in_force"]
            .as_array()
            .unwrap()
            .len(),
        9
    );
    let turns = v["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 17);
    assert!(turns.iter().all(|t| t["connector"].is_array()));
}

#[test]
fn errors_say_whose_fault() {
    use overrules::ErrorKind;
    let kind = |pairs: &[(&str, &str)]| resolve(pairs).unwrap_err().kind();
    assert_eq!(kind(&[("segment", "zzzz")]), ErrorKind::NotFound);
    assert_eq!(
        kind(&[("segment", "e26d3384"), ("at", "2")]),
        ErrorKind::InvalidInput
    );
    assert_eq!(
        kind(&[("segment", "e26d3384"), ("mode", "tractor")]),
        ErrorKind::InvalidInput
    );
}

#[test]
fn stats_find_and_network() {
    let net = net();
    assert_eq!(api::stats(&net)["segments"], 6);
    let hits = api::find(&net, "Pintor");
    assert_eq!(hits[0]["id"], "1b946971-1141-444f-ba2e-63f1a41808ae");
    let geojson: Value = serde_json::from_str(&api::network_geojson(&net)).unwrap();
    assert_eq!(geojson["features"].as_array().unwrap().len(), 6);
}
