//! The page, embedded so that a single binary can serve it.

/// `index.html`.
pub const INDEX_HTML: &str = include_str!("../web/index.html");
/// `app.js`: the page logic.
pub const APP_JS: &str = include_str!("../web/app.js");
/// `backend.js`: picks HTTP or WebAssembly to answer the page.
pub const BACKEND_JS: &str = include_str!("../web/backend.js");
/// `style.css`.
pub const STYLE_CSS: &str = include_str!("../web/style.css");

/// Every file of the page, as `(path, content type, body)`; `""` is the index.
pub const FILES: [(&str, &str, &str); 4] = [
    ("", "text/html; charset=utf-8", INDEX_HTML),
    ("app.js", "text/javascript; charset=utf-8", APP_JS),
    ("backend.js", "text/javascript; charset=utf-8", BACKEND_JS),
    ("style.css", "text/css; charset=utf-8", STYLE_CSS),
];
