//! Rule evaluation, as the Overture guide "Scoping and travel modes" specifies it:
//!
//! 1. A rule **matches** when *all* of its scoping conditions hold (logical AND).
//! 2. For speed limits and access restrictions, the **determining rule** is the *last*
//!    matching rule in the list ([`last_match`]). For prohibited transitions, every matching
//!    rule is in force ([`all_matches`]).
//! 3. With no matching rule, a property-specific default applies.
//!
//! Evaluation is three-valued. A condition can hold, fail, or be *undetermined* because
//! the [`Situation`] does not supply the fact it depends on (a `during` rule with no time
//! of travel, say). Undetermined rules never determine a value, but they are reported, so
//! a caller can see which facts would change the answer.
//!
//! Every interpretation this module has to make where the spec is silent is listed in
//! `docs/semantics.md`.

use std::collections::HashMap;
use std::fmt;

use chrono::{Datelike, NaiveDateTime};
use opening_hours::{OpeningHours, RuleKind};
use opening_hours_syntax::rules::day::WeekDayRange;
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
    evaluate_with(index, rule, s, None)
}

fn evaluate_with<R: Scoped + ?Sized>(
    index: usize,
    rule: &R,
    s: &Situation,
    times: Option<&TimeCache>,
) -> RuleEvaluation {
    let checks = collect_checks(rule, s, times, true);
    RuleEvaluation {
        index,
        verdict: checks.verdict,
        scopes: checks.scopes.unwrap_or_default(),
    }
}

pub(crate) fn verdict<R: Scoped>(rule: &R, s: &Situation, times: &TimeCache) -> Verdict {
    collect_checks(rule, s, Some(times), false).verdict
}

fn collect_checks<R: Scoped + ?Sized>(
    rule: &R,
    s: &Situation,
    times: Option<&TimeCache>,
    explain: bool,
) -> Checks {
    let mut checks = Checks {
        verdict: Verdict::Match,
        scopes: explain.then(Vec::new),
    };
    if let Some(b) = rule.between() {
        check_between(b, s.at, &mut checks);
    }
    if let Some(w) = rule.when() {
        check_when(w, s, times, &mut checks);
    }
    checks
}

/// Resolves a single-valued rule list (access, speed limits): the last match wins.
///
/// This low-level function expects a valid situation. Call [`Situation::validate`] first,
/// or use [`crate::resolve`], which validates before evaluating a segment's rules.
pub fn last_match<R: Scoped>(rules: &[R], s: &Situation) -> Resolution {
    last_match_with(rules, s, None)
}

pub(crate) fn last_match_with<R: Scoped>(
    rules: &[R],
    s: &Situation,
    times: Option<&TimeCache>,
) -> Resolution {
    let rules: Vec<RuleEvaluation> = rules
        .iter()
        .enumerate()
        .map(|(i, r)| evaluate_with(i, r, s, times))
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
    all_matches_with(rules, s, None)
}

pub(crate) fn all_matches_with<R: Scoped>(
    rules: &[R],
    s: &Situation,
    times: Option<&TimeCache>,
) -> SetResolution {
    let rules: Vec<RuleEvaluation> = rules
        .iter()
        .enumerate()
        .map(|(i, r)| evaluate_with(i, r, s, times))
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

struct Checks {
    verdict: Verdict,
    scopes: Option<Vec<ScopeCheck>>,
}

impl Checks {
    fn push(
        &mut self,
        scope: Scope,
        verdict: Verdict,
        requires: impl FnOnce() -> String,
        because: impl FnOnce() -> String,
    ) {
        self.verdict = Verdict::all([self.verdict, verdict]);
        if let Some(scopes) = &mut self.scopes {
            scopes.push(ScopeCheck {
                scope,
                verdict,
                requires: requires(),
                because: because(),
            });
        }
    }
}

fn check_when(w: &When, s: &Situation, times: Option<&TimeCache>, out: &mut Checks) {
    if let Some(h) = w.heading {
        check_heading(h, s.heading, out);
    }
    if let Some(expr) = &w.during {
        check_during(expr, s.time, times, out);
    }
    if let Some(modes) = &w.mode {
        check_mode(modes, s.mode.as_ref(), out);
    }
    if let Some(req) = &w.using {
        check_claims(Scope::Using, req, &s.using, out);
    }
    if let Some(req) = &w.recognized {
        check_claims(Scope::Recognized, req, &s.recognized, out);
    }
    for v in w.vehicle.iter().flatten() {
        check_vehicle(v, s, out);
    }
}

fn check_between(b: &Between, at: Option<f64>, out: &mut Checks) {
    let verdict = at.map_or(Verdict::Undetermined, |f| yes_no(b.contains(f)));
    out.push(
        Scope::Between,
        verdict,
        || {
            let (lo, hi) = b.bounds();
            format!("[{lo:.3}, {hi:.3}]")
        },
        || match at {
            None => "no position given".into(),
            Some(f) => {
                let note = if b.is_inverted() {
                    format!(" (range written inverted as {b})")
                } else {
                    String::new()
                };
                let place = if verdict == Verdict::Match {
                    "inside"
                } else {
                    "outside"
                };
                format!("position {f:.3} is {place}{note}")
            }
        },
    );
}

fn check_heading(required: Heading, have: Option<Heading>, out: &mut Checks) {
    let verdict = have.map_or(Verdict::Undetermined, |h| yes_no(h == required));
    out.push(
        Scope::Heading,
        verdict,
        || required.to_string(),
        || have.map_or_else(|| "no heading given".into(), |h| format!("travelling {h}")),
    );
}

pub(crate) type TimeCache = HashMap<String, PreparedTime>;

#[derive(Debug, Clone)]
pub(crate) struct PreparedTime {
    parsed: Result<(OpeningHours, Option<String>), String>,
    missing_context: Option<&'static str>,
}

impl PreparedTime {
    pub(crate) fn new(expr: &str) -> Self {
        let parsed = parse_opening_hours(expr);
        let missing_context = parsed.as_ref().ok().and_then(|(oh, _)| {
            let expression = oh.get_expression();
            if expression.rules.iter().any(|r| {
                r.day_selector
                    .weekday
                    .iter()
                    .any(|d| matches!(d, WeekDayRange::Holiday { .. }))
            }) {
                Some("holiday calendar not provided")
            } else if expression.rules.iter().any(|r| {
                r.time_selector.spans.iter().any(|s| {
                    s.range.start.as_time_event().is_some() || s.range.end.as_time_event().is_some()
                })
            }) {
                Some("location and time zone for solar events not provided")
            } else {
                None
            }
        });
        Self {
            parsed,
            missing_context,
        }
    }
}

fn check_during(
    expr: &str,
    time: Option<NaiveDateTime>,
    times: Option<&TimeCache>,
    out: &mut Checks,
) {
    let Some(t) = time else {
        out.push(
            Scope::During,
            Verdict::Undetermined,
            || expr.into(),
            || "no time of travel given".into(),
        );
        return;
    };
    let uncached;
    let prepared = if let Some(prepared) = times.and_then(|cache| cache.get(expr)) {
        prepared
    } else {
        uncached = PreparedTime::new(expr);
        &uncached
    };
    let supported_date = (1900..10_000).contains(&t.year());
    let verdict = match &prepared.parsed {
        Ok((oh, _)) if prepared.missing_context.is_none() && supported_date => {
            match oh.state(t).0 {
                RuleKind::Open => Verdict::Match,
                RuleKind::Closed => Verdict::NoMatch,
                RuleKind::Unknown => Verdict::Undetermined,
            }
        }
        _ => Verdict::Undetermined,
    };
    out.push(
        Scope::During,
        verdict,
        || expr.into(),
        || {
            if let Err(e) = &prepared.parsed {
                return format!("cannot parse opening_hours: {e}");
            }
            let note = prepared
                .parsed
                .as_ref()
                .ok()
                .and_then(|(_, r)| r.as_ref())
                .map(|r| format!(" (read \"{expr}\" as \"{r}\")"))
                .unwrap_or_default();
            if let Some(reason) = prepared.missing_context {
                return format!("{reason}{note}");
            }
            if !supported_date {
                return "time is outside supported years 1900..=9999".into();
            }
            let when = t.format("%a %Y-%m-%d %H:%M");
            match verdict {
                Verdict::Match => format!("{when} is inside{note}"),
                Verdict::NoMatch => format!("{when} is outside{note}"),
                Verdict::Undetermined => format!("opening_hours evaluates to 'unknown'{note}"),
            }
        },
    );
}

/// Parses an `opening_hours` expression. If it does not parse as written, tries once more
/// with [`repair_opening_hours`]; the repaired text is returned so the caller can say so.
fn parse_opening_hours(expr: &str) -> Result<(OpeningHours, Option<String>), String> {
    match expr.parse::<OpeningHours>() {
        Ok(oh) => Ok((oh, None)),
        Err(original) => {
            let repaired = repair_opening_hours(expr).ok_or_else(|| original.to_string())?;
            let oh = repaired
                .parse::<OpeningHours>()
                .map_err(|_| original.to_string())?;
            Ok((oh, Some(repaired)))
        }
    }
}

/// Restores the separators Overture data is missing in some `opening_hours` values:
///
/// * a space between days and times: `Mo-Fr07:00-18:00` → `Mo-Fr 07:00-18:00`;
/// * a comma between consecutive time ranges: `07:00-09:0014:00-16:00` →
///   `07:00-09:00,14:00-16:00`;
/// * a comma between consecutive days: `SaSu10:00-13:00` → `Sa,Su 10:00-13:00`.
///
/// Returns `None` if none of these patterns occurs.
fn repair_opening_hours(expr: &str) -> Option<String> {
    const DAYS: [&str; 9] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su", "PH", "SH"];
    let chars: Vec<char> = expr.chars().collect();
    let is_day = |at: usize| {
        chars.get(at..at + 2).is_some_and(|pair| {
            let pair: String = pair.iter().collect();
            DAYS.contains(&pair.as_str())
        })
    };
    let mut out = String::with_capacity(expr.len() + 4);
    let mut changed = false;
    for (i, &c) in chars.iter().enumerate() {
        let after_day = i >= 2 && is_day(i - 2);
        // `…HH:MM` immediately followed by another digit.
        let after_time = i >= 5
            && chars[i - 5].is_ascii_digit()
            && chars[i - 4].is_ascii_digit()
            && chars[i - 3] == ':'
            && chars[i - 2].is_ascii_digit()
            && chars[i - 1].is_ascii_digit();
        if c.is_ascii_digit() && after_day {
            out.push(' ');
            changed = true;
        } else if (c.is_ascii_digit() && after_time) || (after_day && is_day(i)) {
            out.push(',');
            changed = true;
        }
        out.push(c);
    }
    changed.then_some(out)
}

fn check_mode(modes: &[TravelMode], have: Option<&TravelMode>, out: &mut Checks) {
    let hit = have.and_then(|m| modes.iter().find(|r| m.is_within(r)));
    let narrower = have.and_then(|m| modes.iter().find(|r| r.is_within(m)));
    let verdict = if hit.is_some() {
        Verdict::Match
    } else if have.is_none() || narrower.is_some() {
        Verdict::Undetermined
    } else {
        Verdict::NoMatch
    };
    out.push(
        Scope::Mode,
        verdict,
        || format!("[{}]", join(modes)),
        || {
            let Some(m) = have else {
                return "no travel mode given".into();
            };
            if let Some(hit) = hit {
                let how = if hit == m {
                    "exactly".into()
                } else {
                    format!("{m} ⊂ {hit}")
                };
                format!("{m} is covered ({how})")
            } else if let Some(narrower) = narrower {
                format!("rule names {narrower}, narrower than {m}; give a more specific mode")
            } else {
                format!("{m} is not covered")
            }
        },
    );
}

/// `using` and `recognized` are claims: a traveller who declares none holds none.
fn check_claims(scope: Scope, required: &[String], have: &[String], out: &mut Checks) {
    let hit = required.iter().find(|r| have.contains(r));
    out.push(
        scope,
        yes_no(hit.is_some()),
        || format!("[{}]", required.join(", ")),
        || {
            if let Some(hit) = hit {
                format!("traveller declared {hit}")
            } else if have.is_empty() {
                "traveller declared none".into()
            } else {
                format!("traveller declared [{}]", have.join(", "))
            }
        },
    );
}

fn check_vehicle(v: &VehicleSelector, s: &Situation, out: &mut Checks) {
    let threshold = v.canonical_value();
    let have = s.vehicle.get(&v.dimension);
    let verdict = match (&threshold, have) {
        (Ok(threshold), Some(&x)) => yes_no(v.comparison.holds(x, *threshold)),
        _ => Verdict::Undetermined,
    };
    out.push(
        Scope::Vehicle,
        verdict,
        || v.to_string(),
        || match (threshold, have) {
            (Err(reason), _) => reason,
            (Ok(_), None) => format!("no vehicle {} given", v.dimension),
            (Ok(_), Some(&x)) => {
                let unit = v
                    .dimension
                    .quantity()
                    .canonical_unit()
                    .map(|u| format!(" {u}"))
                    .unwrap_or_default();
                format!("vehicle {} is {x}{unit}", v.dimension)
            }
        },
    );
}

fn yes_no(value: bool) -> Verdict {
    if value {
        Verdict::Match
    } else {
        Verdict::NoMatch
    }
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
    use crate::model::{AccessRestriction, AccessType, Comparison, Dimension, SpeedLimit, Unit};

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

    #[test]
    fn repairs_missing_separators_in_opening_hours() {
        assert_eq!(
            repair_opening_hours("Mo-Fr07:00-18:00").as_deref(),
            Some("Mo-Fr 07:00-18:00")
        );
        assert_eq!(
            repair_opening_hours("Fr17:00-23:00; Sa-Su12:00-23:00").as_deref(),
            Some("Fr 17:00-23:00; Sa-Su 12:00-23:00")
        );
        assert_eq!(
            repair_opening_hours("Mo-Fr07:00-09:0014:00-16:00").as_deref(),
            Some("Mo-Fr 07:00-09:00,14:00-16:00")
        );
        assert_eq!(
            repair_opening_hours("Mo-Fr10:00-15:00; SaSu10:00-13:00").as_deref(),
            Some("Mo-Fr 10:00-15:00; Sa,Su 10:00-13:00")
        );
        assert_eq!(
            repair_opening_hours("Tu-ThSu12:00-22:00").as_deref(),
            Some("Tu-Th,Su 12:00-22:00")
        );
        assert_eq!(repair_opening_hours("Mo-Fr 07:00-18:00"), None);
        assert_eq!(repair_opening_hours("08:00-20:00"), None);
    }

    #[test]
    fn a_repaired_time_rule_applies_and_says_so() {
        let rule = SpeedLimit {
            max_speed: None,
            min_speed: None,
            is_max_speed_variable: None,
            when: Some(When {
                during: Some("Mo-Fr07:00-18:00".into()),
                ..When::default()
            }),
            between: None,
        };
        let tuesday = evaluate(0, &rule, &Situation::new().time(at(13, 8, 30)));
        assert_eq!(tuesday.verdict, Verdict::Match);
        assert!(
            tuesday.scopes[0]
                .because
                .contains("read \"Mo-Fr07:00-18:00\" as \"Mo-Fr 07:00-18:00\"")
        );
        let sunday = evaluate(0, &rule, &Situation::new().time(at(18, 8, 30)));
        assert_eq!(sunday.verdict, Verdict::NoMatch);
    }

    #[test]
    fn an_unreadable_time_rule_stays_undetermined() {
        let rule = SpeedLimit {
            max_speed: None,
            min_speed: None,
            is_max_speed_variable: None,
            when: Some(When {
                during: Some("whenever".into()),
                ..When::default()
            }),
            between: None,
        };
        let e = evaluate(0, &rule, &Situation::new().time(at(13, 8, 30)));
        assert_eq!(e.verdict, Verdict::Undetermined);
        assert!(
            e.scopes[0]
                .because
                .starts_with("cannot parse opening_hours")
        );
    }
}
