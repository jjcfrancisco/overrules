//! The explorer's resolver, compiled to WebAssembly so the page can run without a server.
//!
//! Every method returns JSON text produced by [`overrules_explorer::api`], so the static
//! page and `overrules serve` give the same answers.

use std::collections::HashMap;

use overrules::{Network, pack};
use overrules_explorer::api;
use wasm_bindgen::prelude::*;

/// A network loaded from a pack, answering the page's four calls.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Explorer {
    net: Network,
    source: String,
}

#[wasm_bindgen]
impl Explorer {
    /// Loads a pack written by `overrules pack` (already decompressed).
    #[wasm_bindgen(constructor)]
    pub fn new(pack: &[u8]) -> Result<Self, JsError> {
        let decoded = pack::decode(pack)?;
        Ok(Self {
            net: Network::new(decoded.segments),
            source: decoded.source,
        })
    }

    /// The label the pack was written with.
    pub fn source(&self) -> String {
        self.source.clone()
    }

    /// See [`api::stats`].
    pub fn stats(&self) -> String {
        api::stats(&self.net).to_string()
    }

    /// See [`api::network_geojson`].
    pub fn network(&self) -> String {
        api::network_geojson(&self.net)
    }

    /// See [`api::find`].
    pub fn find(&self, q: &str) -> String {
        api::find(&self.net, q).to_string()
    }

    /// See [`api::resolve`]. `params` is a JSON object of query-string parameters, e.g.
    /// `{"segment": "99698abc", "at": "0.9"}`.
    pub fn resolve(&self, params: &str) -> Result<String, JsError> {
        let p: HashMap<String, String> = serde_json::from_str(params)?;
        Ok(api::resolve(&self.net, &p)?.to_string())
    }
}
