#![cfg(feature = "parquet")]

use std::path::{Path, PathBuf};

use overrules::{Error, io};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn nested_parquet_records_match_geojson_for_both_codecs() {
    let expected = io::read_path(fixture("nyc.geojson")).unwrap();
    for name in ["nyc-snappy.parquet", "nyc-zstd.parquet"] {
        let actual = io::read_path(fixture(name)).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(&expected) {
            assert_eq!(
                io::geojson::to_feature(a),
                io::geojson::to_feature(e),
                "{name}, {}",
                e.id
            );
        }
    }
}

#[test]
fn invalid_nested_record_fails_the_entire_read_with_its_path() {
    let error = io::read_path(fixture("invalid-record.parquet")).unwrap_err();
    match error {
        Error::InvalidRecord { id, path, reason } => {
            assert_eq!(id, "11f153a8-1c4e-4b79-b1b6-90f83745c26b");
            assert_eq!(path, "access_restrictions[0].access_type");
            assert!(reason.contains("permissive"));
        }
        other => panic!("expected InvalidRecord, got {other:?}"),
    }
}

#[test]
fn invalid_wkb_fails_the_entire_read_with_the_segment_id() {
    let error = io::read_path(fixture("invalid-wkb.parquet")).unwrap_err();
    match error {
        Error::InvalidGeometry { id, reason } => {
            assert_eq!(id, "cae646b2-ead0-4295-819c-cafba9dc072d");
            assert!(reason.starts_with("WKB:"));
        }
        other => panic!("expected InvalidGeometry, got {other:?}"),
    }
}
