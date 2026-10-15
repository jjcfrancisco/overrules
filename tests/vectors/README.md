# Conformance vectors (draft)

Each file here is a suite: a fixture of real segments, and cases that say what the Overture
spec implies for a given traveller. `cargo test` runs every suite (see `../vectors.rs`).

The aim is a set of vectors that any implementation (Rust, Python, JavaScript, a router) can
run, so they agree on what the data means. The format is a draft and will change before 1.0.

```jsonc
{
  "fixture": "../fixtures/nyc.geojson",   // relative to this file, GeoJSON or Parquet
  "cases": [
    {
      "name": "what this case shows",
      "segment": "<full GERS id>",
      "situation": {                          // every field optional
        "at": 0.05,                           // linear reference, 0..1
        "heading": "forward",                 // forward | backward
        "mode": "car",
        "time": "2026-10-13T08:30:00",        // local time
        "using": ["to_deliver"],
        "recognized": ["as_private"],
        "vehicle": { "weight": 12 }           // metres, metric tonnes, or a count
      },
      "expect": {                             // only the fields given are checked
        "access":        { "determining": 1, "could_override": [], "value": "denied" },
        "speed_limit":   { "determining": 4, "could_override": [], "max_speed": { "value": 20, "unit": "mph" } },
        "prohibited_transitions": { "in_force": [0, 3], "could_apply": [] }
      }
    }
  ]
}
```

- `determining` is the index of the rule that decided, or `null` when none matched.
- `could_override` lists undetermined rules that would win if their missing facts held.
- `value: null` for access means no rule matched and the class default applies.

The semantics behind each expectation are in [`docs/semantics.md`](../../docs/semantics.md).
