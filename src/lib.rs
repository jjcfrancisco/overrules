//! Evaluate [Overture Maps](https://overturemaps.org) transportation rules for any
//! traveller, and say which rule decided.
//!
//! Overture attaches `speed_limits`, `access_restrictions` and `prohibited_transitions` to
//! road segments as ordered lists of scoped rules. A rule can be limited by `when`
//! (heading, time, travel mode, purpose, status, vehicle dimensions) and by `between` (a
//! stretch of the segment, as a fraction of its length).
//!
//! * A rule applies when all its conditions hold.
//! * For speed and access, the last matching rule wins. For prohibited turns, every matching
//!   rule applies.
//! * With no matching rule, a property default applies.
//! * Positions along segments use WGS84 geodesic distances.
//!
//! Every rule is evaluated, and each condition says why it held or failed. Facts you did not
//! supply leave a rule undetermined rather than guessed.
//!
//! # Example
//!
//! ```
//! use geo::line_string;
//! use overrules::model::{AccessRestriction, When};
//! use overrules::{Access, AccessType, Heading, Segment, Situation, TravelMode};
//!
//! // A street closed to motor vehicles travelling backward, except residents.
//! let mut street = Segment::new("demo", line_string![(x: -73.99, y: 40.73), (x: -73.98, y: 40.73)])?;
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
//!
//! # Schema version
//!
//! The crate reads data written to Overture schema [`SCHEMA_VERSION`].

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

/// The version of the [Overture schema](https://github.com/OvertureMaps/schema) this crate
/// reads.
///
/// This is the target version of the supported transportation fields, not a full schema
/// validator or a version detected from an input file. Unknown values of the strict enums
/// fail to load with [`Error::InvalidRecord`] naming the segment and field.
/// Unknown travel modes are retained as [`TravelMode::Other`]; unmodelled fields are ignored.
/// Which schema a release uses is listed in Overture's
/// [release calendar](https://docs.overturemaps.org/release-calendar.json) and in each release's
/// [STAC catalogue](https://stac.overturemaps.org) entry (`schema:version`).
pub const SCHEMA_VERSION: &str = "2.0.0";

pub use answer::{Access, Answer, resolve};
pub use error::{Error, ErrorKind, Result};
pub use eval::{Resolution, SetResolution, Verdict};
pub use model::{AccessType, Dimension, Heading, Segment, TravelMode};
pub use network::{Hit, Network, RuleStats, Stats};
pub use situation::Situation;

/// Compiles the README's Rust examples as doc tests.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
