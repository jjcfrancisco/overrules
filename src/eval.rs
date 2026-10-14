//! Rule evaluation, as the Overture guide "Scoping and travel modes" specifies it:
//!
//! 1. a rule **matches** when *all* of its scoping conditions hold (logical AND);
//! 2. the **determining rule** is the *last* matching rule in the list;
//! 3. with no matching rule, a property-specific default applies.
//!
//! Evaluation is three-valued. A condition can hold, fail, or be *undetermined* because
//! the [`Situation`] does not supply the fact it depends on (a `during` rule with no time
//! of travel, say). Undetermined rules never determine a value, but they are reported, so
//! a caller can see which facts would change the answer.
//!
//! Every interpretation this module has to make where the spec is silent is listed in
//! `docs/semantics.md`.

use std::fmt;

use chrono::NaiveDateTime;
use opening_hours::{OpeningHours, RuleKind};
use serde::Serialize;

use crate::Situation;
use crate::model::{Between, Heading, Scoped, TravelMode, VehicleSelector, When};

/// The outcome of checking one condition, or one rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The condition holds (or, for a rule, all its conditions hold).
    Match,
    /// The condition fails (or, for a rule, at least one does).
    NoMatch,
    /// A fact the condition needs was not given (and, for a rule, none failed).
    Undetermined,
}

impl Verdict {
    /// Combines condition verdicts the way a rule does: any failure fails the rule;
    /// otherwise any unknown leaves it undetermined.
    pub fn all(verdicts: impl IntoIterator<Item = Self>) -> Self {
        verdicts
            .into_iter()
            .fold(Self::Match, |acc, v| match (acc, v) {
                (Self::NoMatch, _) | (_, Self::NoMatch) => Self::NoMatch,
                (Self::Undetermined, _) | (_, Self::Undetermined) => Self::Undetermined,
                (Self::Match, Self::Match) => Self::Match,
            })
    }
}

/// Which scoping property a condition comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// `between`: position along the segment.
    Between,
    /// `when.heading`: direction of travel.
    Heading,
    /// `when.during`: time of travel.
    During,
    /// `when.mode`: travel mode.
    Mode,
    /// `when.using`: purpose of use.
    Using,
    /// `when.recognized`: recognised status.
    Recognized,
    /// `when.vehicle`: one vehicle dimension.
    Vehicle,
}

impl Scope {
    /// The property name as Overture spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Between => "between",
            Self::Heading => "heading",
            Self::During => "during",
            Self::Mode => "mode",
            Self::Using => "using",
            Self::Recognized => "recognized",
            Self::Vehicle => "vehicle",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

/// One condition of a rule, checked against the situation, with the reason.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScopeCheck {
    /// Which scoping property.
    pub scope: Scope,
    /// What the rule requires, rendered, e.g. `[motor_vehicle]` or `weight > 3.5 t`.
    pub requires: String,
    /// Whether it holds.
    pub verdict: Verdict,
    /// Why, in words, e.g. `car ⊂ motor_vehicle`.
    pub because: String,
}

/// One rule, evaluated.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuleEvaluation {
    /// Position of the rule in its list.
    pub index: usize,
    /// The rule's overall verdict.
    pub verdict: Verdict,
    /// Each of its conditions. Empty for an unconditional rule.
    pub scopes: Vec<ScopeCheck>,
}

/// How a single-valued property (access, speed limit) resolved: every rule evaluated,
/// and which one decided.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resolution {
    /// Every rule, in list order.
    pub rules: Vec<RuleEvaluation>,
    /// Index of the determining rule: the last one that matched.
    pub determining: Option<usize>,
    /// Undetermined rules after the determining one (or all of them, if none
    /// determined). Any of them would take over if its missing facts held.
    pub could_override: Vec<usize>,
}

impl Resolution {
    /// The determining rule, looked up in the list that was resolved.
    pub fn winner<'r, R>(&self, rules: &'r [R]) -> Option<&'r R> {
        self.determining.and_then(|i| rules.get(i))
    }

    /// Whether the answer is final: no undetermined rule could still override it.
    pub fn is_settled(&self) -> bool {
        self.could_override.is_empty()
    }
}

/// How a set-valued property (prohibited transitions) resolved: every matching rule is
/// in force at once, not just the last.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SetResolution {
    /// Every rule, in list order.
    pub rules: Vec<RuleEvaluation>,
    /// Indices of the rules in force.
    pub in_force: Vec<usize>,
    /// Indices of the rules that are undetermined: in force if their missing facts hold.
    pub could_apply: Vec<usize>,
}

/// Evaluates one rule against a situation.
pub fn evaluate<R: Scoped + ?Sized>(index: usize, rule: &R, s: &Situation) -> RuleEvaluation {
    let mut scopes = Vec::new();
    if let Some(b) = rule.between() {
        scopes.push(check_between(b, s.at));
    }
    if let Some(w) = rule.when() {
        check_when(w, s, &mut scopes);
    }
    RuleEvaluation {
        index,
        verdict: Verdict::all(scopes.iter().map(|c| c.verdict)),
        scopes,
    }
}

/// Resolves a single-valued rule list (access, speed limits): the last match wins.
///
/// This low-level function expects a valid situation. Call [`Situation::validate`] first,
/// or use [`crate::resolve`], which validates before evaluating a segment's rules.
pub fn last_match<R: Scoped>(rules: &[R], s: &Situation) -> Resolution {
    let rules: Vec<RuleEvaluation> = rules
        .iter()
        .enumerate()
        .map(|(i, r)| evaluate(i, r, s))
        .collect();
    let determining = rules
        .iter()
        .rev()
        .find(|e| e.verdict == Verdict::Match)
        .map(|e| e.index);
    let could_override = rules
        .iter()
        .filter(|e| e.verdict == Verdict::Undetermined && determining.is_none_or(|d| e.index > d))
        .map(|e| e.index)
        .collect();
    Resolution {
        rules,
        determining,
        could_override,
    }
}

/// Resolves a set-valued rule list (prohibited transitions): every match is in force.
///
/// Like [`last_match`], this expects a valid situation; it does not validate the facts.
pub fn all_matches<R: Scoped>(rules: &[R], s: &Situation) -> SetResolution {
    let rules: Vec<RuleEvaluation> = rules
        .iter()
        .enumerate()
        .map(|(i, r)| evaluate(i, r, s))
        .collect();
    let with = |v| {
        rules
            .iter()
            .filter(|e| e.verdict == v)
            .map(|e| e.index)
            .collect()
    };
    SetResolution {
        in_force: with(Verdict::Match),
        could_apply: with(Verdict::Undetermined),
        rules,
    }
}

fn check(
    scope: Scope,
    requires: impl Into<String>,
    verdict: Verdict,
    because: impl Into<String>,
) -> ScopeCheck {
    ScopeCheck {
        scope,
        requires: requires.into(),
        verdict,
        because: because.into(),
    }
}

fn check_when(w: &When, s: &Situation, out: &mut Vec<ScopeCheck>) {
    if let Some(h) = w.heading {
        out.push(check_heading(h, s.heading));
    }
    if let Some(expr) = &w.during {
        out.push(check_during(expr, s.time));
    }
    if let Some(modes) = &w.mode {
        out.push(check_mode(modes, s.mode.as_ref()));
    }
    if let Some(req) = &w.using {
        out.push(check_claims(Scope::Using, req, &s.using));
    }
    if let Some(req) = &w.recognized {
        out.push(check_claims(Scope::Recognized, req, &s.recognized));
    }
    for v in w.vehicle.iter().flatten() {
        out.push(check_vehicle(v, s));
    }
}

fn check_between(b: &Between, at: Option<f64>) -> ScopeCheck {
    let (lo, hi) = b.bounds();
    let requires = format!("[{lo:.3}, {hi:.3}]");
    let note = if b.is_inverted() {
        format!(" (range written inverted as {b})")
    } else {
        String::new()
    };
    match at {
        None => check(
            Scope::Between,
            requires,
            Verdict::Undetermined,
            "no position given",
        ),
        Some(f) if b.contains(f) => check(
            Scope::Between,
            requires,
            Verdict::Match,
            format!("position {f:.3} is inside{note}"),
        ),
        Some(f) => check(
            Scope::Between,
            requires,
            Verdict::NoMatch,
            format!("position {f:.3} is outside{note}"),
        ),
    }
}

fn check_heading(required: Heading, have: Option<Heading>) -> ScopeCheck {
    let (verdict, because) = match have {
        None => (Verdict::Undetermined, "no heading given".to_owned()),
        Some(h) if h == required => (Verdict::Match, format!("travelling {h}")),
        Some(h) => (Verdict::NoMatch, format!("travelling {h}")),
    };
    check(Scope::Heading, required.as_str(), verdict, because)
}

fn check_during(expr: &str, time: Option<NaiveDateTime>) -> ScopeCheck {
    let (verdict, because) = match time {
        None => (Verdict::Undetermined, "no time of travel given".to_owned()),
        Some(t) => match expr.parse::<OpeningHours>() {
            Err(e) => (
                Verdict::Undetermined,
                format!("cannot parse opening_hours: {e}"),
            ),
            Ok(oh) => {
                let when = t.format("%a %Y-%m-%d %H:%M");
                match oh.state(t).0 {
                    RuleKind::Open => (Verdict::Match, format!("{when} is inside")),
                    RuleKind::Closed => (Verdict::NoMatch, format!("{when} is outside")),
                    RuleKind::Unknown => (
                        Verdict::Undetermined,
                        "opening_hours evaluates to 'unknown'".to_owned(),
                    ),
                }
            }
        },
    };
    check(Scope::During, expr, verdict, because)
}

fn check_mode(modes: &[TravelMode], have: Option<&TravelMode>) -> ScopeCheck {
    let requires = format!("[{}]", join(modes));
    let (verdict, because) = match have {
        None => (Verdict::Undetermined, "no travel mode given".to_owned()),
        Some(m) => {
            if let Some(hit) = modes.iter().find(|r| m.is_within(r)) {
                let how = if hit == m {
                    "exactly".to_owned()
                } else {
                    format!("{m} ⊂ {hit}")
                };
                (Verdict::Match, format!("{m} is covered ({how})"))
            } else if let Some(narrower) = modes.iter().find(|r| r.is_within(m)) {
                // The traveller gave a group (motor_vehicle); the rule names one of its
                // members (car). Whether it applies depends on which member they are.
                (
                    Verdict::Undetermined,
                    format!("rule names {narrower}, narrower than {m}; give a more specific mode"),
                )
            } else {
                (Verdict::NoMatch, format!("{m} is not covered"))
            }
        }
    };
    check(Scope::Mode, requires, verdict, because)
}

/// `using` and `recognized` are claims: a traveller who declares none holds none.
fn check_claims(scope: Scope, required: &[String], have: &[String]) -> ScopeCheck {
    let requires = format!("[{}]", required.join(", "));
    let (verdict, because) = if let Some(hit) = required.iter().find(|r| have.contains(r)) {
        (Verdict::Match, format!("traveller declared {hit}"))
    } else if have.is_empty() {
        (Verdict::NoMatch, "traveller declared none".to_owned())
    } else {
        (
            Verdict::NoMatch,
            format!("traveller declared [{}]", have.join(", ")),
        )
    };
    check(scope, requires, verdict, because)
}

fn check_vehicle(v: &VehicleSelector, s: &Situation) -> ScopeCheck {
    let requires = v.to_string();
    let (verdict, because) = match (v.canonical_value(), s.vehicle.get(&v.dimension)) {
        (Err(reason), _) => (Verdict::Undetermined, reason),
        (Ok(_), None) => (
            Verdict::Undetermined,
            format!("no vehicle {} given", v.dimension),
        ),
        (Ok(threshold), Some(&x)) => {
            let unit = v
                .dimension
                .quantity()
                .canonical_unit()
                .map(|u| format!(" {u}"))
                .unwrap_or_default();
            let verdict = if v.comparison.holds(x, threshold) {
                Verdict::Match
            } else {
                Verdict::NoMatch
            };
            (verdict, format!("vehicle {} is {x}{unit}", v.dimension))
        }
    };
    check(Scope::Vehicle, requires, verdict, because)
}

fn join<T: fmt::Display>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::model::{AccessRestriction, AccessType, Comparison, Dimension, Unit};

    fn access(access_type: AccessType, when: When) -> AccessRestriction {
        AccessRestriction {
            access_type,
            when: Some(when),
            between: None,
        }
    }

    fn at(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, d)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    #[test]
    fn last_matching_rule_wins() {
        let rules = [
            access(
                AccessType::Denied,
                When {
                    mode: Some(vec![TravelMode::MotorVehicle]),
                    ..When::default()
                },
            ),
            access(
                AccessType::Allowed,
                When {
                    mode: Some(vec![TravelMode::MotorVehicle]),
                    during: Some("Mo-Fr 07:00-11:00".into()),
                    ..When::default()
                },
            ),
        ];
        let car = Situation::new().mode(TravelMode::Car);

        let tuesday = last_match(&rules, &car.clone().time(at(13, 8, 30)));
        assert_eq!(tuesday.determining, Some(1));
        assert_eq!(
            tuesday.winner(&rules).map(|r| r.access_type),
            Some(AccessType::Allowed)
        );

        let sunday = last_match(&rules, &car.clone().time(at(18, 8, 30)));
        assert_eq!(sunday.determining, Some(0));

        // No time given: rule 1 is undetermined and flagged as a possible override.
        let unknown = last_match(&rules, &car);
        assert_eq!(unknown.determining, Some(0));
        assert_eq!(unknown.could_override, vec![1]);
        assert!(!unknown.is_settled());
    }

    #[test]
    fn heading_and_between() {
        let rule = AccessRestriction {
            access_type: AccessType::Denied,
            when: Some(When {
                heading: Some(Heading::Backward),
                ..When::default()
            }),
            between: Some(Between::new(0.2, 0.6)),
        };
        let s = Situation::new().heading(Heading::Backward);
        assert_eq!(
            evaluate(0, &rule, &s.clone().at(0.5)).verdict,
            Verdict::Match
        );
        assert_eq!(
            evaluate(0, &rule, &s.clone().at(0.7)).verdict,
            Verdict::NoMatch
        );
        assert_eq!(evaluate(0, &rule, &s).verdict, Verdict::Undetermined);
    }

    #[test]
    fn a_failure_beats_an_unknown() {
        let rule = access(
            AccessType::Denied,
            When {
                heading: Some(Heading::Forward),
                during: Some("Mo-Fr 07:00-11:00".into()),
                ..When::default()
            },
        );
        let s = Situation::new().heading(Heading::Backward);
        assert_eq!(evaluate(0, &rule, &s).verdict, Verdict::NoMatch);
    }

    #[test]
    fn a_group_mode_is_undetermined_against_a_narrower_rule() {
        let rule = access(
            AccessType::Denied,
            When {
                mode: Some(vec![TravelMode::Car]),
                ..When::default()
            },
        );
        let s = Situation::new().mode(TravelMode::MotorVehicle);
        assert_eq!(evaluate(0, &rule, &s).verdict, Verdict::Undetermined);
        let s = Situation::new().mode(TravelMode::Foot);
        assert_eq!(evaluate(0, &rule, &s).verdict, Verdict::NoMatch);
    }

    #[test]
    fn claims_absent_mean_not_held() {
        let rule = access(
            AccessType::Allowed,
            When {
                recognized: Some(vec!["as_private".into()]),
                ..When::default()
            },
        );
        assert_eq!(
            evaluate(0, &rule, &Situation::new()).verdict,
            Verdict::NoMatch
        );
        let private = Situation::new().recognized("as_private");
        assert_eq!(evaluate(0, &rule, &private).verdict, Verdict::Match);
    }

    #[test]
    fn vehicle_thresholds_are_compared_in_canonical_units() {
        let rule = access(
            AccessType::Denied,
            When {
                vehicle: Some(vec![VehicleSelector {
                    dimension: Dimension::Weight,
                    comparison: Comparison::GreaterThan,
                    value: 7_000.0,
                    unit: Some(Unit::Kg),
                }]),
                ..When::default()
            },
        );
        let check =
            |t: f64| evaluate(0, &rule, &Situation::new().vehicle(Dimension::Weight, t)).verdict;
        assert_eq!(check(7.0), Verdict::NoMatch);
        assert_eq!(check(7.5), Verdict::Match);
        assert_eq!(
            evaluate(0, &rule, &Situation::new()).verdict,
            Verdict::Undetermined
        );
    }

    #[test]
    fn every_matching_transition_is_in_force() {
        let rules = [
            access(AccessType::Denied, When::default()),
            access(AccessType::Denied, When::default()),
        ];
        let set = all_matches(&rules, &Situation::new());
        assert_eq!(set.in_force, vec![0, 1]);
    }
}
