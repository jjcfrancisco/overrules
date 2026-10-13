//! WebAssembly entry point: the same resolver, running in the browser tab.
//!
//! Every method returns JSON text produced by [`crate::api`], so the static
//! page and `gers-lr serve` give byte-for-byte identical answers.

use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use crate::api;
use crate::index::Network;
use crate::pack;

#[wasm_bindgen]
pub struct Gers {
    net: Network,
    source: String,
}

#[wasm_bindgen]
impl Gers {
    /// Load a pack produced by `gers-lr pack` (already gunzipped).
    #[wasm_bindgen(constructor)]
    pub fn new(pack: &[u8]) -> Result<Gers, JsError> {
        let (segments, source) = pack::decode(pack).map_err(err)?;
        Ok(Gers {
            net: Network::new(segments),
            source,
        })
    }

    pub fn source(&self) -> String {
        self.source.clone()
    }

    pub fn stats(&self) -> String {
        api::stats(&self.net).to_string()
    }

    pub fn network(&self) -> String {
        api::network_geojson(&self.net)
    }

    pub fn find(&self, q: &str) -> String {
        api::find(&self.net, q).to_string()
    }

    /// `params` is a JSON object of query-string parameters, e.g. `{"segment":"99698abc","at":"0.9"}`.
    pub fn resolve(&self, params: &str) -> Result<String, JsError> {
        let p: HashMap<String, String> =
            serde_json::from_str(params).map_err(|e| JsError::new(&format!("params: {e}")))?;
        api::resolve(&self.net, &p)
            .map(|v| v.to_string())
            .map_err(err)
    }
}

fn err(e: anyhow::Error) -> JsError {
    JsError::new(&format!("{e:#}"))
}
