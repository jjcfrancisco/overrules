# Test fixtures

`nyc.geojson` holds eight real New York City segments from the Overture Maps `2026-09-23.1`
release, chosen because each shows one rule-evaluation case (see `../vectors/nyc.json`). It was
written by the `export` example and is regenerated with `just fixtures`.

`nyc-snappy.parquet` and `nyc-zstd.parquet` encode those same records with WKB geometry
and nested Parquet fields. `invalid-record.parquet` changes the third record's access type
to an unsupported value. `invalid-wkb.parquet` truncates the second record's geometry.
Rebuild all four with `python3 scripts/make_parquet_fixtures.py` (requires the DuckDB CLI).
These files test both codecs and ensure failures after valid rows reject the whole read.

`time-context.geojson` contains synthetic rules for missing holiday and solar context.

Data © OpenStreetMap contributors and Overture Maps Foundation, available under the
[Open Database License](https://opendatacommons.org/licenses/odbl/1-0/) (ODbL).
