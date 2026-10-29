//! Public API regressions found during the 0.2 review.

use chrono::NaiveDate;
use geo::{Bearing, Geodesic, Point, line_string};
use overrules::model::{AccessRestriction, Between, When};
use overrules::{AccessType, Network, Segment, Situation, Verdict};

fn segment() -> Segment {
    Segment::new(
        "road",
        line_string![(x: -74.0, y: 40.7), (x: -73.9999, y: 40.7)],
    )
    .expect("valid test geometry")
}

#[test]
fn time_rules_require_holiday_and_solar_context() {
    let mut segment = segment();
    let time = NaiveDate::from_ymd_opt(2026, 12, 25)
        .unwrap()
        .and_hms_opt(18, 0, 0)
        .unwrap();
    for expression in [
        "PH",
        "SH",
        "sunrise-sunset",
        "sunset-sunrise",
        "Mo-Su; PH off",
    ] {
        segment.access_restrictions = vec![AccessRestriction {
            access_type: AccessType::Denied,
            between: None,
            when: Some(When {
                during: Some(expression.into()),
                ..When::default()
            }),
        }];
        let answer = overrules::resolve(&segment, &Situation::new().time(time)).unwrap();
        assert_eq!(
            answer.access.rules[0].verdict,
            Verdict::Undetermined,
            "{expression}"
        );
        assert!(!answer.access.is_settled());
    }
}

#[test]
fn nearest_search_continues_past_misleading_boxes() {
    let mut segments: Vec<_> = (0..40)
        .map(|i| {
            Segment::new(
                format!("decoy-{i}"),
                line_string![(x: -0.01, y: -0.01), (x: -0.01, y: 0.01), (x: 0.01, y: 0.01)],
            )
            .unwrap()
        })
        .collect();
    segments.push(
        Segment::new(
            "nearest",
            line_string![(x: 0.0001, y: -0.001), (x: 0.0001, y: 0.001)],
        )
        .unwrap(),
    );
    let network = Network::new(segments);
    assert_eq!(
        network.snap(Point::new(0.0, 0.0)).unwrap().segment.id,
        "nearest"
    );
}

#[test]
fn invalid_coordinates_return_errors() {
    assert!(
        Segment::new(
            "invalid",
            line_string![(x: 0.0, y: 91.0), (x: 0.1, y: 91.0)]
        )
        .is_err()
    );
    let network = Network::new(vec![segment()]);
    for point in [
        Point::new(f64::NAN, 0.0),
        Point::new(0.0, 91.0),
        Point::new(181.0, 0.0),
    ] {
        assert!(network.snap(point).is_err());
    }
}

#[test]
fn projected_boundaries_still_match() {
    let mut segment = segment();
    for fraction in [0.1, 0.2, 0.3, 0.5, 0.7, 0.8, 0.9] {
        let point = segment.line().point_at(fraction).point();
        let projected = segment.line().project(point).unwrap();
        for between in [Between::new(0.0, fraction), Between::new(fraction, 1.0)] {
            segment.access_restrictions = vec![AccessRestriction {
                access_type: AccessType::Denied,
                between: Some(between),
                when: None,
            }];
            let answer =
                overrules::resolve(&segment, &Situation::new().at(projected.at.fraction)).unwrap();
            assert_eq!(
                answer.access.rules[0].verdict,
                Verdict::Match,
                "{fraction}: {projected:?}"
            );
        }
    }
}

#[test]
fn bearing_is_local_to_the_position() {
    let segment = Segment::new(
        "polar",
        line_string![(x: -45.0, y: 80.0), (x: 45.0, y: 80.0)],
    )
    .unwrap();
    for fraction in [0.1, 0.5, 0.9] {
        let at = segment.line().point_at(fraction);
        let next = segment.line().point_at(fraction + 1e-6);
        let expected = Geodesic.bearing(at.point(), next.point()).rem_euclid(360.0);
        assert!((segment.line().bearing_at(&at) - expected).abs() < 0.001);
    }
}

#[test]
fn equal_slice_has_a_valid_linestring() {
    let segment = segment();
    assert!(Segment::new("slice", segment.line().slice(0.5, 0.5)).is_ok());
}

#[test]
fn collection_members_must_be_features() {
    let mut feature = overrules::io::geojson::to_feature(&segment());
    for kind in [serde_json::Value::Null, serde_json::json!("NotAFeature")] {
        feature["type"] = kind;
        let collection = serde_json::json!({"type": "FeatureCollection", "features": [feature]});
        assert!(overrules::io::geojson::from_value(collection).is_err());
    }
}

#[test]
fn nearest_matches_an_exhaustive_search_worldwide() {
    let segments: Vec<_> = (-8..=8)
        .flat_map(|lat| (-17..=17).map(move |lon| (lat, lon)))
        .map(|(lat, lon)| {
            let (x, y) = (f64::from(lon) * 10.0, f64::from(lat) * 10.0);
            Segment::new(
                format!("{lat}/{lon}"),
                line_string![(x: x, y: y), (x: x + 0.1, y: y + 0.1)],
            )
            .unwrap()
        })
        .chain([
            Segment::new(
                "date-line",
                line_string![(x: 179.99, y: 1.0), (x: -179.99, y: 1.0)],
            )
            .unwrap(),
            Segment::new(
                "pole",
                line_string![(x: -90.0, y: 89.0), (x: 90.0, y: 89.0)],
            )
            .unwrap(),
        ])
        .collect();
    let network = Network::new(segments);
    for point in [
        Point::new(0.01, 0.01),
        Point::new(180.0, 1.0),
        Point::new(-180.0, 1.0),
        Point::new(0.0, 89.9),
        Point::new(-130.0, -79.99),
    ] {
        let mut expected: Vec<_> = network
            .segments()
            .iter()
            .map(|s| (s.line().project(point).unwrap().offset_m, &s.id))
            .collect();
        expected.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        let hits = network.nearest(point, 5).unwrap();
        let ids: Vec<_> = hits.iter().map(|h| &h.segment.id).collect();
        assert_eq!(
            ids,
            expected
                .iter()
                .take(5)
                .map(|(_, id)| *id)
                .collect::<Vec<_>>(),
            "{point:?}"
        );
    }
    assert!(network.nearest(Point::new(0.0, 0.0), 0).unwrap().is_empty());
    assert!(network.nearest(Point::new(f64::NAN, 0.0), 0).is_err());
    assert!(
        Network::new(vec![])
            .nearest(Point::new(0.0, 91.0), 1)
            .is_err()
    );
}

#[test]
fn bearings_and_slices_handle_endpoints_and_duplicate_vertices() {
    let line = Segment::new(
        "duplicates",
        line_string![(x: 0.0, y: 0.0), (x: 0.0, y: 0.0), (x: 0.01, y: 0.0), (x: 0.01, y: 0.0)],
    )
    .unwrap();
    for fraction in [0.0, 0.5, 1.0] {
        assert!((line.line().bearing_at(&line.line().point_at(fraction)) - 90.0).abs() < 1e-6);
        assert!(Segment::new("slice", line.line().slice(fraction, fraction)).is_ok());
    }
    let zero = Segment::new("zero", line_string![(x: 0.0, y: 0.0), (x: 0.0, y: 0.0)]).unwrap();
    assert_eq!(zero.line().bearing_at(&zero.line().point_at(0.5)), 0.0);
    assert!(Segment::new("slice", zero.line().slice(0.0, 1.0)).is_ok());
    assert!(zero.line().project(Point::new(f64::INFINITY, 0.0)).is_err());
}

#[test]
fn time_context_checks_ignore_comments_and_preserve_failed_scopes() {
    let mut segment = segment();
    let time = NaiveDate::from_ymd_opt(2026, 12, 25)
        .unwrap()
        .and_hms_opt(18, 0, 0)
        .unwrap();
    for (expression, heading, expected) in [
        ("24/7 \"PH sunrise\"", None, Verdict::Match),
        ("PH", Some(overrules::Heading::Backward), Verdict::NoMatch),
        ("PH07:00-19:00", None, Verdict::Undetermined),
        ("whenever", None, Verdict::Undetermined),
    ] {
        segment.access_restrictions = vec![AccessRestriction {
            access_type: AccessType::Denied,
            when: Some(When {
                during: Some(expression.into()),
                heading,
                ..When::default()
            }),
            between: None,
        }];
        let prepared = overrules::PreparedSegment::new(&segment);
        let situation = Situation::new()
            .time(time)
            .heading(overrules::Heading::Forward);
        for _ in 0..2 {
            let detailed = prepared.resolve(&situation).unwrap();
            let compact = prepared.resolve_compact(&situation).unwrap();
            assert_eq!(detailed.access.rules[0].verdict, expected, "{expression}");
            assert_eq!(compact.access.determining, detailed.access.determining);
            assert_eq!(
                compact.access.could_override,
                detailed.access.could_override
            );
        }
        assert!(
            prepared
                .resolve_compact(&Situation::new().at(f64::NAN))
                .is_err()
        );
    }
}

#[test]
fn extreme_dates_stay_undetermined_instead_of_panicking() {
    let mut segment = segment();
    segment.access_restrictions.push(AccessRestriction {
        access_type: AccessType::Denied,
        when: Some(When {
            during: Some("24/7".into()),
            ..When::default()
        }),
        between: None,
    });
    for time in [chrono::NaiveDateTime::MIN, chrono::NaiveDateTime::MAX] {
        let answer = overrules::resolve(&segment, &Situation::new().time(time)).unwrap();
        assert_eq!(answer.access.rules[0].verdict, Verdict::Undetermined);
    }
}
