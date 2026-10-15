//! "Standing here, travelling like this: what applies?" for one segment.

use serde::{Serialize, Serializer};

use crate::eval::{self, Resolution, SetResolution};
use crate::model::{AccessType, ProhibitedTransition, Segment, SpeedLimit};
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
    situation.validate()?;
    Ok(Answer {
        segment,
        situation: situation.clone(),
        access: eval::last_match(&segment.access_restrictions, situation),
        speed_limit: eval::last_match(&segment.speed_limits, situation),
        prohibited_transitions: eval::all_matches(&segment.prohibited_transitions, situation),
    })
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
