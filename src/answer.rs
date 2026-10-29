//! "Standing here, travelling like this: what applies?" for one segment.

use serde::{Serialize, Serializer};

use crate::eval::{self, PreparedTime, Resolution, SetResolution, TimeCache, Verdict};
use crate::model::{AccessType, ProhibitedTransition, Scoped, Segment, SpeedLimit};
use crate::{Result, Situation};

/// Everything that applies to a traveller on one segment, with the reasoning.
///
/// Serialises with the segment as its id, for logs and JSON front ends.
#[derive(Debug, Clone, Serialize)]
pub struct Answer<'a> {
    /// The segment that was asked about.
    #[serde(serialize_with = "segment_id")]
    pub segment: &'a Segment,
    /// The facts it was evaluated against.
    pub situation: Situation,
    /// `access_restrictions`, resolved.
    pub access: Resolution,
    /// `speed_limits`, resolved.
    pub speed_limit: Resolution,
    /// `prohibited_transitions`, resolved.
    pub prohibited_transitions: SetResolution,
}

/// What `access_restrictions` say for the traveller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "access_type")]
pub enum Access {
    /// A rule decided.
    Rule(AccessType),
    /// No rule matched. Overture leaves the default to the class of road and the
    /// application: a footway is not open to cars just because no rule says otherwise.
    Default,
}

/// Resolves every rule-based property of `segment` for `situation`.
///
/// Fails only if the situation itself is invalid (see [`Situation::validate`]).
///
/// ```
/// # use overrules::{Segment, Situation, TravelMode, Heading, Access, AccessType};
/// # use overrules::model::{AccessRestriction, When};
/// # use geo::line_string;
/// let mut seg = Segment::new("demo", line_string![(x: -73.99, y: 40.73), (x: -73.98, y: 40.73)])?;
/// seg.access_restrictions.push(AccessRestriction {
///     access_type: AccessType::Denied,
///     when: Some(When { heading: Some(Heading::Backward), ..When::default() }),
///     between: None,
/// });
/// let answer = overrules::resolve(&seg, &Situation::new().heading(Heading::Backward))?;
/// assert_eq!(answer.access_outcome(), Access::Rule(AccessType::Denied));
/// # Ok::<(), overrules::Error>(())
/// ```
pub fn resolve<'a>(segment: &'a Segment, situation: &Situation) -> Result<Answer<'a>> {
    PreparedSegment::new(segment).resolve(situation)
}

/// Resolves rules without building explanation strings or cloning the situation.
/// For repeated queries, retain a [`PreparedSegment`] to also reuse parsed time expressions.
pub fn resolve_compact<'a>(
    segment: &'a Segment,
    situation: &Situation,
) -> Result<CompactAnswer<'a>> {
    PreparedSegment::new(segment).resolve_compact(situation)
}

/// A borrowed segment with each distinct time expression parsed once.
///
/// Parse failures and separator repairs are cached too. Borrowing the segment prevents
/// its rules changing while this cache is in use. No global cache or lock is needed.
///
/// ```
/// # use overrules::{Segment, Situation, PreparedSegment};
/// # use geo::line_string;
/// let segment = Segment::new("road", line_string![(x: 0.0, y: 0.0), (x: 0.01, y: 0.0)])?;
/// let prepared = PreparedSegment::new(&segment);
/// let answer = prepared.resolve_compact(&Situation::new())?;
/// assert!(answer.access.is_settled());
/// # Ok::<(), overrules::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct PreparedSegment<'a> {
    segment: &'a Segment,
    times: TimeCache,
}

impl<'a> PreparedSegment<'a> {
    /// Parses the segment's time expressions, preserving errors as undetermined conditions.
    pub fn new(segment: &'a Segment) -> Self {
        let expressions = segment
            .access_restrictions
            .iter()
            .filter_map(Scoped::when)
            .chain(segment.speed_limits.iter().filter_map(Scoped::when))
            .chain(
                segment
                    .prohibited_transitions
                    .iter()
                    .filter_map(Scoped::when),
            )
            .filter_map(|w| w.during.as_ref());
        let mut times = TimeCache::new();
        for expression in expressions {
            times
                .entry(expression.clone())
                .or_insert_with(|| PreparedTime::new(expression));
        }
        Self { segment, times }
    }

    /// Resolves all rules with explanations and validates the supplied situation.
    pub fn resolve(&self, situation: &Situation) -> Result<Answer<'a>> {
        situation.validate()?;
        Ok(Answer {
            segment: self.segment,
            situation: situation.clone(),
            access: eval::last_match_with(
                &self.segment.access_restrictions,
                situation,
                Some(&self.times),
            ),
            speed_limit: eval::last_match_with(
                &self.segment.speed_limits,
                situation,
                Some(&self.times),
            ),
            prohibited_transitions: eval::all_matches_with(
                &self.segment.prohibited_transitions,
                situation,
                Some(&self.times),
            ),
        })
    }

    /// Resolves all rules without collecting explanations. Uncertainty is still retained.
    pub fn resolve_compact(&self, situation: &Situation) -> Result<CompactAnswer<'a>> {
        situation.validate()?;
        let mut transitions = CompactSetResolution {
            in_force: Vec::new(),
            could_apply: Vec::new(),
        };
        for (i, rule) in self.segment.prohibited_transitions.iter().enumerate() {
            match eval::verdict(rule, situation, &self.times) {
                Verdict::Match => transitions.in_force.push(i),
                Verdict::Undetermined => transitions.could_apply.push(i),
                Verdict::NoMatch => {}
            }
        }
        Ok(CompactAnswer {
            segment: self.segment,
            access: self.choice(&self.segment.access_restrictions, situation),
            speed_limit: self.choice(&self.segment.speed_limits, situation),
            prohibited_transitions: transitions,
        })
    }

    fn choice<R: Scoped>(&self, rules: &[R], situation: &Situation) -> CompactResolution {
        let mut result = CompactResolution {
            determining: None,
            could_override: Vec::new(),
        };
        for (i, rule) in rules.iter().enumerate() {
            match eval::verdict(rule, situation, &self.times) {
                Verdict::Match => {
                    result.determining = Some(i);
                    result.could_override.clear();
                }
                Verdict::Undetermined => result.could_override.push(i),
                Verdict::NoMatch => {}
            }
        }
        result
    }
}

/// A single-valued resolution without individual rule explanations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactResolution {
    /// The last rule known to match.
    pub determining: Option<usize>,
    /// Later undetermined rules, or all undetermined rules if none matched.
    pub could_override: Vec<usize>,
}

impl CompactResolution {
    /// Whether no undetermined rule could override the current choice.
    pub fn is_settled(&self) -> bool {
        self.could_override.is_empty()
    }
}

/// A set-valued resolution without individual rule explanations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactSetResolution {
    /// Every rule known to match.
    pub in_force: Vec<usize>,
    /// Every undetermined rule.
    pub could_apply: Vec<usize>,
}

/// Rule choices and uncertainty without explanation strings or a copied situation.
#[derive(Debug, Clone, Serialize)]
pub struct CompactAnswer<'a> {
    /// The segment that was asked about. Serialised as its id.
    #[serde(serialize_with = "segment_id")]
    pub segment: &'a Segment,
    /// Access restriction choice.
    pub access: CompactResolution,
    /// Speed limit choice.
    pub speed_limit: CompactResolution,
    /// All applicable prohibited transitions.
    pub prohibited_transitions: CompactSetResolution,
}

impl<'a> CompactAnswer<'a> {
    /// The known access choice. Check `access.is_settled()` before treating it as final.
    pub fn access_outcome(&self) -> Access {
        self.access
            .determining
            .and_then(|i| self.segment.access_restrictions.get(i))
            .map_or(Access::Default, |r| Access::Rule(r.access_type))
    }

    /// The last known matching speed rule, if any.
    pub fn speed_limit_rule(&self) -> Option<&'a SpeedLimit> {
        self.speed_limit
            .determining
            .and_then(|i| self.segment.speed_limits.get(i))
    }

    /// The prohibited transitions known to apply.
    pub fn transitions_in_force(&self) -> impl Iterator<Item = &'a ProhibitedTransition> + '_ {
        self.prohibited_transitions
            .in_force
            .iter()
            .filter_map(|&i| self.segment.prohibited_transitions.get(i))
    }
}

impl<'a> Answer<'a> {
    /// What access restrictions say.
    pub fn access_outcome(&self) -> Access {
        self.access
            .winner(&self.segment.access_restrictions)
            .map_or(Access::Default, |r| Access::Rule(r.access_type))
    }

    /// The speed-limit rule that applies, if any did.
    pub fn speed_limit_rule(&self) -> Option<&'a SpeedLimit> {
        self.speed_limit.winner(&self.segment.speed_limits)
    }

    /// The prohibited transitions in force.
    pub fn transitions_in_force(&self) -> impl Iterator<Item = &'a ProhibitedTransition> + '_ {
        let rules = &self.segment.prohibited_transitions;
        self.prohibited_transitions
            .in_force
            .iter()
            .filter_map(|&i| rules.get(i))
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes `&T`, and T is a reference
fn segment_id<S: Serializer>(segment: &&Segment, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&segment.id)
}
