//! Runs the conformance vectors in `tests/vectors/*.json` against the fixtures they name.

use std::fs;
use std::path::{Path, PathBuf};

use overrules::{Access, Network, Situation};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Suite {
    fixture: PathBuf,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    segment: String,
    situation: Situation,
    expect: serde_json::Map<String, Value>,
}

/// What `overrules` answered, in the vocabulary of the vectors.
fn actual(net: &Network, case: &Case) -> serde_json::Map<String, Value> {
    let segment = net.lookup(&case.segment).expect("segment in fixture");
    let a = overrules::resolve(segment, &case.situation).expect("valid situation");
    let access = match a.access_outcome() {
        Access::Rule(t) => json!(t),
        Access::Default => Value::Null,
    };
    let Value::Object(map) = json!({
        "access": {
            "determining": a.access.determining,
            "could_override": a.access.could_override,
            "value": access,
        },
        "speed_limit": {
            "determining": a.speed_limit.determining,
            "could_override": a.speed_limit.could_override,
            "max_speed": a.speed_limit_rule().and_then(|r| r.max_speed),
        },
        "prohibited_transitions": {
            "in_force": a.prohibited_transitions.in_force,
            "could_apply": a.prohibited_transitions.could_apply,
        },
    }) else {
        unreachable!()
    };
    map
}

/// Every field the vector names must equal ours; fields it leaves out are not checked.
fn mismatches(expected: &Value, actual: &Value, path: &str, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (k, v) in e {
                mismatches(
                    v,
                    a.get(k).unwrap_or(&Value::Null),
                    &format!("{path}.{k}"),
                    out,
                );
            }
        }
        (Value::Number(e), Value::Number(a)) if e.as_f64() == a.as_f64() => {}
        (e, a) if e == a => {}
        (e, a) => out.push(format!("{path}: expected {e}, got {a}")),
    }
}

fn run_suite(path: &Path) -> Vec<String> {
    let suite: Suite = serde_json::from_str(&fs::read_to_string(path).expect("read vectors"))
        .expect("parse vectors");
    let fixture = path.parent().expect("vectors dir").join(&suite.fixture);
    let segments = overrules::io::read_path(&fixture).expect("read fixture");
    let net = Network::new(segments);
    let mut failures = Vec::new();
    for case in &suite.cases {
        let mut out = Vec::new();
        mismatches(
            &Value::Object(case.expect.clone()),
            &Value::Object(actual(&net, case)),
            "",
            &mut out,
        );
        failures.extend(
            out.into_iter()
                .map(|m| format!("{}: {}{m}", path.display(), case.name)),
        );
    }
    failures
}

#[test]
fn conformance_vectors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let mut suites: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("vectors dir")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    suites.sort();
    assert!(!suites.is_empty(), "no vector files in {}", dir.display());
    let failures: Vec<String> = suites.iter().flat_map(|p| run_suite(p)).collect();
    assert!(
        failures.is_empty(),
        "{} failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
