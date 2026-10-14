//! The error type for everything this crate can fail at.

/// Errors returned by `overrules`.
///
/// Variants describe *what* went wrong. Front ends that need to decide *whose* fault it
/// was (a 400 or a 500, say) should ask [`Error::kind`] instead of matching variants.
///
/// Readers fail the whole read rather than returning a partial network. Property decoding
/// errors retain the segment ID and field path, including inside GeoJSON collections.
/// This enum is non-exhaustive, so callers need a fallback arm:
///
/// ```no_run
/// use overrules::{Error, ErrorKind, io};
///
/// match io::read_path("segment.geojson") {
///     Ok(segments) => println!("Loaded {} segments", segments.len()),
///     Err(Error::InvalidRecord { id, path, reason }) => {
///         eprintln!("Segment {id}, field {path}: {reason}");
///     }
///     Err(error) if error.kind() == ErrorKind::NotFound => {
///         eprintln!("Input file not found: {error}");
///     }
///     Err(error) => eprintln!("Could not read segments: {error}"),
/// }
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No segment has this id, or none starts with this prefix.
    #[error("no segment with id (prefix) {0:?}")]
    SegmentNotFound(String),

    /// An id prefix matched more than one segment.
    #[error("{matches} segments match id prefix {prefix:?}; give more characters")]
    AmbiguousSegmentId {
        /// The prefix that was looked up.
        prefix: String,
        /// How many segments it matched.
        matches: usize,
    },

    /// The network has no segments, so there is nothing to snap to.
    #[error("the network is empty")]
    EmptyNetwork,

    /// A segment geometry cannot be linearly referenced.
    #[error("segment {id}: {reason}")]
    InvalidGeometry {
        /// GERS id of the offending segment.
        id: String,
        /// What is wrong with it.
        reason: String,
    },

    /// A segment's properties do not match the schema this crate reads
    /// ([`SCHEMA_VERSION`](crate::SCHEMA_VERSION)): typically a value newer than the crate.
    #[error(
        "segment {id}: cannot read `{path}`: {reason} (overrules reads Overture schema {})",
        crate::SCHEMA_VERSION
    )]
    InvalidRecord {
        /// GERS id of the segment, or `?` if it has none.
        id: String,
        /// Where in the properties, e.g. `access_restrictions[2].access_type`.
        path: String,
        /// What is wrong with the value there.
        reason: String,
    },

    /// A linear reference outside `0..=1`.
    #[error("linear reference {0} is outside 0..=1")]
    FractionOutOfRange(f64),

    /// A value that does not parse as the named kind of thing.
    #[error("invalid {what} {value:?}: {reason}")]
    Invalid {
        /// What was being parsed, e.g. `"heading"`.
        what: &'static str,
        /// The offending input.
        value: String,
        /// Why it was rejected, including what would have been accepted.
        reason: String,
    },

    /// A GeoJSON document that is not an Overture segment `Feature`/`FeatureCollection`.
    #[error("GeoJSON: {0}")]
    GeoJson(String),

    /// [`std::io::Error`].
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// [`serde_json::Error`].
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// [`parquet::errors::ParquetError`].
    #[cfg(feature = "parquet")]
    #[error(transparent)]
    Parquet(#[from] parquet::errors::ParquetError),
}

impl Error {
    /// Classifies the error for a caller that has to map it onto a transport.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::SegmentNotFound(_) => ErrorKind::NotFound,
            Self::AmbiguousSegmentId { .. }
            | Self::FractionOutOfRange(_)
            | Self::Invalid { .. }
            | Self::EmptyNetwork => ErrorKind::InvalidInput,
            Self::Io(e) if e.kind() == std::io::ErrorKind::NotFound => ErrorKind::NotFound,
            Self::InvalidGeometry { .. }
            | Self::InvalidRecord { .. }
            | Self::GeoJson(_)
            | Self::Io(_)
            | Self::Json(_) => ErrorKind::Data,
            #[cfg(feature = "parquet")]
            Self::Parquet(_) => ErrorKind::Data,
        }
    }

    pub(crate) fn invalid(
        what: &'static str,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::Invalid {
            what,
            value: value.into(),
            reason: reason.into(),
        }
    }
}

/// How a caller should treat an [`Error`], independent of which variant it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The thing asked for does not exist.
    NotFound,
    /// The question was malformed or out of range; asking differently can succeed.
    InvalidInput,
    /// The input data could not be read or does not look like Overture data.
    Data,
}

/// `Result` with this crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
