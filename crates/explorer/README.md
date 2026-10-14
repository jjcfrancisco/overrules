# overrules-explorer

The web explorer for [`overrules`](https://github.com/jjcfrancisco/overrules): a map page that
asks "what applies here, and why?" for any Overture road, and the JSON API behind it.

- `api`: the page's four calls (`stats`, `network`, `find`, `resolve`) as JSON, independent of
  transport.
- `assets`: the page (`web/`), embedded.
- `server` (feature `server`, on by default): the page and API over HTTP with axum.

The `overrules-wasm` crate compiles the same API to WebAssembly, so the page can also run as
static files with no server. `web/backend.js` picks whichever is available.
