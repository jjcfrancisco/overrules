//! Evaluate [Overture Maps](https://overturemaps.org) transportation rules for any
//! traveller, and say which rule decided.
//!
//! Overture attaches `speed_limits`, `access_restrictions` and `prohibited_transitions` to
//! road segments as ordered lists of scoped rules. A rule can be limited by `when`
//! (heading, time, travel mode, purpose, status, vehicle dimensions) and by `between` (a
//! stretch of the segment, as geodesic fractions of its length). The spec says:
//!
//! * a rule applies when **all** its conditions hold;
//! * the **last** applicable rule in the list wins;
//! * otherwise a property default applies;
//! * `between` positions are **geodesic on WGS84**, not planar.
//!
//! This crate implements exactly that, with provenance: every rule is evaluated, each
//! condition says why it held or failed, and facts you did not supply leave a rule
//! *undetermined* instead of guessed.
//!
//! # Example
//!
//! ```
//! use geo::line_string;
//! use overrules::model::{AccessRestriction, When};
//! use overrules::{Access, AccessType, Heading, Segment, Situation, TravelMode};
//!
//! // A street closed to motor vehicles travelling backward, except residents.
//! let mut street = Segment::new("demo", line_string![(x: -5.99, y: 37.39), (x: -5.98, y: 37.39)])?;
//! street.access_restrictions = vec![
//!     AccessRestriction {
//!         access_type: AccessType::Denied,
//!         when: Some(When {
//!             heading: Some(Heading::Backward),
//!             mode: Some(vec![TravelMode::MotorVehicle]),
//!             ..When::default()
//!         }),
//!         between: None,
//!     },
//!     AccessRestriction {
//!         access_type: AccessType::Allowed,
//!         when: Some(When { recognized: Some(vec!["as_private".into()]), ..When::default() }),
//!         between: None,
//!     },
//! ];
//!
//! let driver = Situation::new().heading(Heading::Backward).mode(TravelMode::Car);
//! let answer = overrules::resolve(&street, &driver)?;
//! assert_eq!(answer.access_outcome(), Access::Rule(AccessType::Denied));
//! assert_eq!(answer.access.determining, Some(0));
//!
//! let resident = driver.recognized("as_private");
//! let answer = overrules::resolve(&street, &resident)?;
//! assert_eq!(answer.access_outcome(), Access::Rule(AccessType::Allowed));
//! # Ok::<(), overrules::Error>(())
//! ```
//!
//! # Modules
//!
//! * [`model`]: typed Overture segments and rules.
//! * [`lr`]: geodesic linear referencing (fraction ⇄ point), and planar comparisons.
//! * [`eval`]: scope checks and rule resolution.
//! * [`Network`]: lookup by id, name and position.
//! * [`io`]: GeoJSON, and Parquet with feature `parquet`.
//!
//! # Features
//!
//! * `parquet`: read Overture GeoParquet release files.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

mod answer;
mod error;
pub mod eval;
pub mod io;
pub mod lr;
pub mod model;
mod network;
mod situation;

pub use answer::{Access, Answer, resolve};
pub use error::{Error, ErrorKind, Result};
pub use eval::{Resolution, SetResolution, Verdict};
pub use model::{AccessType, Dimension, Heading, Segment, TravelMode};
pub use network::{Hit, Network, RuleStats, Stats};
pub use situation::Situation;

/// Compiles the README's example as a doc test.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
