//! Evaluation of Overture rule-based properties.
//!
//! Semantics follow the Overture guide "Scoping and travel modes":
//! * a rule **matches** when *all* of its scoping properties hold (logical AND);
//! * the **determining rule** is the *last* matching rule in the list;
//! * with no matching rule, a property-specific default applies.
//!
//! The evaluator is three-valued: a scope can be satisfied, violated, or
//! *undetermined* because the caller did not supply the fact it depends on
//! (e.g. a `during` rule with no time of travel). Undetermined rules never
//! determine a value, but they are reported so a consumer can see what *could*
//! change the answer.

use std::collections::BTreeMap;

use chrono::NaiveDateTime;
use opening_hours::{OpeningHours, RuleKind};
use serde::Serialize;

use crate::model::{
    AccessRestrictionRule, ProhibitedTransitionRule, Segment, SpeedLimitRule, VehicleSelector, When,
};

/// The facts about a traveller at a position on a segment.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Situation {
    /// Geodesic linear reference 0..1 along the segment.
    pub fraction: Option<f64>,
    /// "forward" or "backward" relative to segment geometry.
    pub heading: Option<String>,
    /// Travel mode: foot, bicycle, vehicle, motor_vehicle, car, truck, motorcycle, bus, hgv, hov, emergency.
    pub mode: Option<String>,
    /// Local time of travel.
    pub time: Option<NaiveDateTime>,
    /// Purposes of use the traveller claims (as_customer, at_destination, to_deliver, to_farm, for_forestry).
    pub using: Vec<String>,
    /// Statuses the traveller holds (as_permitted, as_private, as_disabled, as_employee, as_student).
    pub recognized: Vec<String>,
    /// Vehicle dimensions: weight (t), height/length/width (m), axle_count.
    pub vehicle: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Match,
    NoMatch,
    Undetermined,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScopeVerdict {
    pub scope: String,
    pub requires: String,
    pub verdict: Verdict,
    pub because: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleEvaluation {
    pub index: usize,
    pub rule: String,
    pub scopes: Vec<ScopeVerdict>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Serialize)]
pub struct PropertyResolution {
    pub property: String,
    pub rules: Vec<RuleEvaluation>,
    /// Index of the determining rule (last match), if any.
    pub determining: Option<usize>,
    /// Effective value, rendered.
    pub value: String,
    /// Rules that come *after* the determining rule and were undetermined: any
    /// of them, if its missing fact held, would override the value.
    pub could_override: Vec<usize>,
    pub note: Option<String>,
}

/// Overture's shallow travel-mode taxonomy: each mode and its ancestors.
pub fn mode_lineage(mode: &str) -> Vec<&'static str> {
    match mode {
        "foot" => vec!["foot"],
        "vehicle" => vec!["vehicle"],
        "bicycle" => vec!["bicycle", "vehicle"],
        "motor_vehicle" => vec!["motor_vehicle", "vehicle"],
        "car" => vec!["car", "motor_vehicle", "vehicle"],
        "truck" => vec!["truck", "motor_vehicle", "vehicle"],
        "motorcycle" => vec!["motorcycle", "motor_vehicle", "vehicle"],
        "bus" => vec!["bus", "motor_vehicle", "vehicle"],
        "hgv" => vec!["hgv", "motor_vehicle", "vehicle"],
        "hov" => vec!["hov", "motor_vehicle", "vehicle"],
        "emergency" => vec!["emergency", "motor_vehicle", "vehicle"],
        _ => vec![],
    }
}

fn is_descendant(child: &str, ancestor: &str) -> bool {
    mode_lineage(child).contains(&ancestor)
}

fn check_heading(when: &When, s: &Situation, out: &mut Vec<ScopeVerdict>) {
    let Some(req) = &when.heading else { return };
    let (verdict, because) = match &s.heading {
        None => (
            Verdict::Undetermined,
            "no heading given (--heading)".to_string(),
        ),
        Some(h) if h == req => (Verdict::Match, format!("travelling {h}")),
        Some(h) => (Verdict::NoMatch, format!("travelling {h}")),
    };
    out.push(ScopeVerdict {
        scope: "heading".into(),
        requires: req.clone(),
        verdict,
        because,
    });
}

fn check_during(when: &When, s: &Situation, out: &mut Vec<ScopeVerdict>) {
    let Some(expr) = &when.during else { return };
    let (verdict, because) = match s.time {
        None => (
            Verdict::Undetermined,
            "no time of travel given (--time)".to_string(),
        ),
        Some(t) => match expr.parse::<OpeningHours>() {
            Err(e) => (
                Verdict::Undetermined,
                format!("cannot parse opening_hours: {e}"),
            ),
            Ok(oh) => match oh.state(t).0 {
                RuleKind::Open => (
                    Verdict::Match,
                    format!("{} is inside", t.format("%a %Y-%m-%d %H:%M")),
                ),
                RuleKind::Closed => (
                    Verdict::NoMatch,
                    format!("{} is outside", t.format("%a %Y-%m-%d %H:%M")),
                ),
                RuleKind::Unknown => (
                    Verdict::Undetermined,
                    "opening_hours evaluates to 'unknown'".into(),
                ),
            },
        },
    };
    out.push(ScopeVerdict {
        scope: "during".into(),
        requires: expr.clone(),
        verdict,
        because,
    });
}

fn check_mode(when: &When, s: &Situation, out: &mut Vec<ScopeVerdict>) {
    let Some(modes) = &when.mode else { return };
    let requires = format!("[{}]", modes.join(", "));
    let (verdict, because) = match &s.mode {
        None => (
            Verdict::Undetermined,
            "no travel mode given (--mode)".to_string(),
        ),
        Some(m) => {
            if let Some(hit) = modes.iter().find(|r| is_descendant(m, r)) {
                let how = if hit == m {
                    "exactly".to_string()
                } else {
                    format!("{m} ⊂ {hit}")
                };
                (Verdict::Match, format!("{m} is covered ({how})"))
            } else if let Some(narrower) = modes.iter().find(|r| is_descendant(r, m)) {
                (
                    Verdict::Undetermined,
                    format!("rule targets {narrower}, which is narrower than {m}; refine --mode"),
                )
            } else {
                (Verdict::NoMatch, format!("{m} is not in the list"))
            }
        }
    };
    out.push(ScopeVerdict {
        scope: "mode".into(),
        requires,
        verdict,
        because,
    });
}

fn check_list(
    scope: &str,
    flag: &str,
    required: &Option<Vec<String>>,
    have: &[String],
    out: &mut Vec<ScopeVerdict>,
) {
    let Some(req) = required else { return };
    let requires = format!("[{}]", req.join(", "));
    let (verdict, because) = if let Some(hit) = req.iter().find(|r| have.contains(r)) {
        (Verdict::Match, format!("traveller declared {hit}"))
    } else if have.is_empty() {
        (
            Verdict::NoMatch,
            format!("none declared; add {flag} to claim one"),
        )
    } else {
        (
            Verdict::NoMatch,
            format!("traveller declared [{}]", have.join(", ")),
        )
    };
    out.push(ScopeVerdict {
        scope: scope.into(),
        requires,
        verdict,
        because,
    });
}

fn check_vehicle(when: &When, s: &Situation, out: &mut Vec<ScopeVerdict>) {
    let Some(sel) = &when.vehicle else { return };
    for v in sel {
        let (verdict, because) = match s.vehicle.get(&v.dimension) {
            None => (
                Verdict::Undetermined,
                format!("no {} given (--vehicle {}=…)", v.dimension, v.dimension),
            ),
            Some(x) => {
                let ok = compare(*x, v);
                (
                    if ok { Verdict::Match } else { Verdict::NoMatch },
                    format!("vehicle {} = {}", v.dimension, x),
                )
            }
        };
        out.push(ScopeVerdict {
            scope: "vehicle".into(),
            requires: v.summary(),
            verdict,
            because,
        });
    }
}

fn compare(x: f64, v: &VehicleSelector) -> bool {
    match v.comparison.as_str() {
        "greater_than" => x > v.value,
        "greater_than_equal" => x >= v.value,
        "equal" => (x - v.value).abs() < 1e-9,
        "less_than" => x < v.value,
        "less_than_equal" => x <= v.value,
        _ => false,
    }
}

fn check_between(between: &Option<Vec<f64>>, s: &Situation, out: &mut Vec<ScopeVerdict>) {
    let Some(b) = between else { return };
    if b.len() != 2 {
        out.push(ScopeVerdict {
            scope: "between".into(),
            requires: format!("{b:?}"),
            verdict: Verdict::Undetermined,
            because: "malformed range".into(),
        });
        return;
    }
    let (lo, hi) = (b[0].min(b[1]), b[0].max(b[1]));
    let requires = format!("[{lo:.3}, {hi:.3}]");
    let (verdict, because) = match s.fraction {
        None => (
            Verdict::Undetermined,
            "no position given (--at or --lon/--lat)".to_string(),
        ),
        Some(f) if f >= lo - 1e-9 && f <= hi + 1e-9 => {
            (Verdict::Match, format!("position {f:.3} is inside"))
        }
        Some(f) => (Verdict::NoMatch, format!("position {f:.3} is outside")),
    };
    out.push(ScopeVerdict {
        scope: "between".into(),
        requires,
        verdict,
        because,
    });
}

/// Evaluate one rule's scopes against a situation.
pub fn evaluate_scopes(
    when: Option<&When>,
    between: &Option<Vec<f64>>,
    s: &Situation,
) -> (Vec<ScopeVerdict>, Verdict) {
    let mut out = Vec::new();
    check_between(between, s, &mut out);
    if let Some(w) = when {
        check_heading(w, s, &mut out);
        check_during(w, s, &mut out);
        check_mode(w, s, &mut out);
        check_list("using", "--using", &w.using, &s.using, &mut out);
        check_list(
            "recognized",
            "--recognized",
            &w.recognized,
            &s.recognized,
            &mut out,
        );
        check_vehicle(w, s, &mut out);
    }
    let verdict = if out.iter().any(|v| v.verdict == Verdict::NoMatch) {
        Verdict::NoMatch
    } else if out.iter().any(|v| v.verdict == Verdict::Undetermined) {
        Verdict::Undetermined
    } else {
        Verdict::Match
    };
    (out, verdict)
}

fn scope_summary(when: Option<&When>, between: &Option<Vec<f64>>) -> String {
    let mut s = String::new();
    if let Some(w) = when {
        if !w.is_empty() {
            s.push_str(&format!(" when {}", w.summary()));
        }
    }
    if let Some(b) = between {
        if b.len() == 2 {
            s.push_str(&format!(" between [{:.3}, {:.3}]", b[0], b[1]));
        }
    }
    if s.is_empty() {
        " (unconditional)".to_string()
    } else {
        s
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_generic<R>(
    property: &str,
    rules: &[R],
    s: &Situation,
    when_of: impl Fn(&R) -> Option<&When>,
    between_of: impl Fn(&R) -> &Option<Vec<f64>>,
    head_of: impl Fn(&R) -> String,
    value_of: impl Fn(&R) -> String,
    default_value: &str,
    note: Option<String>,
) -> PropertyResolution {
    let mut evals = Vec::with_capacity(rules.len());
    for (i, r) in rules.iter().enumerate() {
        let (scopes, verdict) = evaluate_scopes(when_of(r), between_of(r), s);
        evals.push(RuleEvaluation {
            index: i,
            rule: format!("{}{}", head_of(r), scope_summary(when_of(r), between_of(r))),
            scopes,
            verdict,
        });
    }
    let determining = evals
        .iter()
        .rev()
        .find(|e| e.verdict == Verdict::Match)
        .map(|e| e.index);
    let could_override = evals
        .iter()
        .filter(|e| e.verdict == Verdict::Undetermined && determining.is_none_or(|d| e.index > d))
        .map(|e| e.index)
        .collect();
    let value = match determining {
        Some(i) => value_of(&rules[i]),
        None => default_value.to_string(),
    };
    PropertyResolution {
        property: property.into(),
        rules: evals,
        determining,
        value,
        could_override,
        note,
    }
}

pub fn resolve_access(seg: &Segment, s: &Situation) -> PropertyResolution {
    let rules: &[AccessRestrictionRule] = seg.rec.access_restrictions.as_deref().unwrap_or(&[]);
    resolve_generic(
        "access_restrictions",
        rules,
        s,
        |r| r.when.as_ref(),
        |r| &r.between,
        |r| r.access_type.clone(),
        |r| r.access_type.to_uppercase(),
        "IMPLIED (no matching rule → class default; Overture leaves implied modes to the application)",
        Some(format!("class {} / subtype {}", seg.class(), seg.subtype())),
    )
}

fn speed_text(r: &SpeedLimitRule) -> String {
    let mut parts = Vec::new();
    if let Some(mx) = &r.max_speed {
        parts.push(format!("max {} {}", mx.value, mx.unit));
    }
    if let Some(mn) = &r.min_speed {
        parts.push(format!("min {} {}", mn.value, mn.unit));
    }
    if r.is_max_speed_variable == Some(true) {
        parts.push("variable".into());
    }
    if parts.is_empty() {
        "(no value)".into()
    } else {
        parts.join(", ")
    }
}

pub fn resolve_speed(seg: &Segment, s: &Situation) -> PropertyResolution {
    let rules: &[SpeedLimitRule] = seg.rec.speed_limits.as_deref().unwrap_or(&[]);
    resolve_generic(
        "speed_limits",
        rules,
        s,
        |r| r.when.as_ref(),
        |r| &r.between,
        speed_text,
        speed_text,
        "NONE IN DATA (fall back to class/jurisdiction default)",
        None,
    )
}

/// Prohibited transitions are a set, not a single value: every matching rule is in force.
#[derive(Debug, Clone, Serialize)]
pub struct TransitionResolution {
    pub rules: Vec<RuleEvaluation>,
    pub in_force: Vec<usize>,
    pub could_apply: Vec<usize>,
}

pub fn resolve_transitions(
    seg: &Segment,
    s: &Situation,
    describe_segment: impl Fn(&str) -> String,
) -> TransitionResolution {
    let rules: &[ProhibitedTransitionRule] =
        seg.rec.prohibited_transitions.as_deref().unwrap_or(&[]);
    let mut evals = Vec::new();
    for (i, r) in rules.iter().enumerate() {
        let (scopes, verdict) = evaluate_scopes(r.when.as_ref(), &r.between, s);
        let path: Vec<String> = r
            .sequence
            .iter()
            .map(|st| {
                format!(
                    "via {}… onto {}",
                    &st.connector_id[..st.connector_id.len().min(12)],
                    describe_segment(&st.segment_id)
                )
            })
            .collect();
        evals.push(RuleEvaluation {
            index: i,
            rule: format!(
                "no transition {} (final heading {}){}",
                path.join(", then "),
                r.final_heading,
                scope_summary(r.when.as_ref(), &r.between)
            ),
            scopes,
            verdict,
        });
    }
    let in_force = evals
        .iter()
        .filter(|e| e.verdict == Verdict::Match)
        .map(|e| e.index)
        .collect();
    let could_apply = evals
        .iter()
        .filter(|e| e.verdict == Verdict::Undetermined)
        .map(|e| e.index)
        .collect();
    TransitionResolution {
        rules: evals,
        in_force,
        could_apply,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn when(heading: Option<&str>, during: Option<&str>, mode: Option<&[&str]>) -> When {
        When {
            heading: heading.map(String::from),
            during: during.map(String::from),
            mode: mode.map(|m| m.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn last_matching_rule_wins() {
        let rules = vec![
            AccessRestrictionRule {
                access_type: "denied".into(),
                when: Some(when(None, None, Some(&["motor_vehicle"]))),
                between: None,
            },
            AccessRestrictionRule {
                access_type: "allowed".into(),
                when: Some(when(
                    None,
                    Some("Mo-Fr 07:00-11:00"),
                    Some(&["motor_vehicle"]),
                )),
                between: None,
            },
        ];
        let tue_0830 = NaiveDate::from_ymd_opt(2026, 10, 13)
            .unwrap()
            .and_hms_opt(8, 30, 0)
            .unwrap();
        let s = Situation {
            mode: Some("car".into()),
            time: Some(tue_0830),
            ..Default::default()
        };
        let res = resolve_generic(
            "access",
            &rules,
            &s,
            |r| r.when.as_ref(),
            |r| &r.between,
            |r| r.access_type.clone(),
            |r| r.access_type.clone(),
            "implied",
            None,
        );
        assert_eq!(res.determining, Some(1));
        assert_eq!(res.value, "allowed");

        let sun = NaiveDate::from_ymd_opt(2026, 10, 18)
            .unwrap()
            .and_hms_opt(8, 30, 0)
            .unwrap();
        let s = Situation {
            mode: Some("car".into()),
            time: Some(sun),
            ..Default::default()
        };
        let res = resolve_generic(
            "access",
            &rules,
            &s,
            |r| r.when.as_ref(),
            |r| &r.between,
            |r| r.access_type.clone(),
            |r| r.access_type.clone(),
            "implied",
            None,
        );
        assert_eq!(res.determining, Some(0));
        assert_eq!(res.value, "denied");

        // No time given: rule 1 is undetermined and flagged as a possible override.
        let s = Situation {
            mode: Some("car".into()),
            ..Default::default()
        };
        let res = resolve_generic(
            "access",
            &rules,
            &s,
            |r| r.when.as_ref(),
            |r| &r.between,
            |r| r.access_type.clone(),
            |r| r.access_type.clone(),
            "implied",
            None,
        );
        assert_eq!(res.determining, Some(0));
        assert_eq!(res.could_override, vec![1]);
    }

    #[test]
    fn mode_taxonomy() {
        assert!(is_descendant("car", "vehicle"));
        assert!(is_descendant("car", "motor_vehicle"));
        assert!(!is_descendant("bicycle", "motor_vehicle"));
        assert!(is_descendant("bicycle", "vehicle"));
        assert!(!is_descendant("foot", "vehicle"));
    }

    #[test]
    fn heading_and_between() {
        let w = when(Some("backward"), None, None);
        let s = Situation {
            heading: Some("backward".into()),
            fraction: Some(0.5),
            ..Default::default()
        };
        let (_, v) = evaluate_scopes(Some(&w), &Some(vec![0.2, 0.6]), &s);
        assert_eq!(v, Verdict::Match);
        let s = Situation {
            heading: Some("backward".into()),
            fraction: Some(0.7),
            ..Default::default()
        };
        let (_, v) = evaluate_scopes(Some(&w), &Some(vec![0.2, 0.6]), &s);
        assert_eq!(v, Verdict::NoMatch);
    }
}
