//! The overrules web explorer: a map page that asks "what applies here, and why?" for any
//! road, and the JSON API behind it.
//!
//! The API is transport-agnostic. The HTTP server (feature `server`) and the WebAssembly
//! build both call the functions in [`api`], so every front end gives the same answers.
//! The page itself is in [`assets`]; it picks its transport at run time (see
//! `web/backend.js`).

#![warn(missing_docs)]

pub mod api;
pub mod assets;
mod error;
pub mod request;
#[cfg(feature = "server")]
pub mod server;

pub use error::{Error, Result};
pub use request::Request;
