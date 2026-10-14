use overrules::ErrorKind;

/// Errors from answering an explorer request.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A request parameter is missing, malformed or contradictory.
    #[error("{0}")]
    BadRequest(String),
    /// The resolver failed.
    #[error(transparent)]
    Core(#[from] overrules::Error),
}

impl Error {
    /// How a transport should treat the error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::BadRequest(_) => ErrorKind::InvalidInput,
            Self::Core(e) => e.kind(),
        }
    }
}

/// `Result` with the explorer's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
