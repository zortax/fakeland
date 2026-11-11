//! Preset listings, defaults and parameter checks.

use super::*;

#[test]
fn every_name_has_a_preset() {
    let all = all();
    let names: Vec<&str> = all.iter().map(Preset::name).collect();
    assert_eq!(names, NAMES);
    for name in NAMES {
        let p = by_name(name).unwrap();
        assert!(!p.description().is_empty());
        let schema = p.params_schema();
        let props = schema.as_value()["properties"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        assert!(props.contains_key("iterations"), "{name}: {props:?}");
        assert!(props.contains_key("seed"), "{name}");
        let defaults = p.default_params();
        assert!(defaults.is_object(), "{name}");
    }
    assert!(by_name("nope").is_none());
    assert_eq!(list().len(), NAMES.len());
}

#[test]
fn defaults_build_except_startup() {
    for name in NAMES {
        let p = by_name(name).unwrap();
        let built = p.bench(&PresetParams::new());
        if *name == "startup" {
            assert!(
                matches!(built, Err(PresetError::Params { .. })),
                "startup needs a command"
            );
            continue;
        }
        let bench = built.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!bench.analyzer_names().is_empty(), "{name}");
        assert_eq!(bench.name(), *name);
    }
    let startup = by_name("startup")
        .unwrap()
        .bench(&PresetParams::new().set("command", ["true"]))
        .unwrap();
    assert_eq!(startup.analyzer_names(), ["startup", "resources"]);
}

#[test]
fn defaults_round_trip_as_params() {
    for p in all() {
        let defaults = PresetParams::from_value(p.default_params()).unwrap();
        let mut defaults = defaults;
        if p.name() == "startup" {
            defaults = defaults.set("command", ["true"]);
        }
        p.bench(&defaults)
            .unwrap_or_else(|e| panic!("{}: {e}", p.name()));
    }
}

#[test]
fn bad_params_are_errors() {
    let pacing = by_name("pacing").unwrap();
    let e = pacing
        .bench(&PresetParams::new().set("durration", "1s"))
        .unwrap_err();
    assert!(
        matches!(e, PresetError::UnknownParam { ref key, .. } if key == "durration"),
        "{e}"
    );
    let e = pacing
        .bench(&PresetParams::new().set("duration", "0ms"))
        .unwrap_err();
    assert!(matches!(e, PresetError::Params { .. }), "{e}");
    let e = pacing
        .bench(&PresetParams::new().set("duration", [1, 2]))
        .unwrap_err();
    assert!(matches!(e, PresetError::Params { .. }), "{e}");
    let scroll = by_name("scroll-glide").unwrap();
    assert!(
        scroll
            .bench(&PresetParams::new().set("velocity", 0.0))
            .is_err()
    );
    let hover = by_name("hover-latency")
        .unwrap()
        .bench(
            &PresetParams::new()
                .set("iterations", 7)
                .set("repeats", 5)
                .set("detector", "app_mark")
                .set("position", serde_json::json!({"x": 200, "y": 175}))
                .set(
                    "target",
                    serde_json::json!({"type": "app_id", "app_id": "x"}),
                ),
        )
        .unwrap();
    assert_eq!(hover.measured_iterations(), 7);
}

#[test]
fn pairs_parse_as_json_or_text() {
    let p =
        PresetParams::from_pairs(["iterations=5", "rates=[60,144]", "text=hello world"]).unwrap();
    assert_eq!(p.0["iterations"], 5);
    assert_eq!(p.0["rates"], serde_json::json!([60, 144]));
    assert_eq!(p.0["text"], "hello world");
    assert!(PresetParams::from_pairs(["novalue"]).is_err());
    let refresh = by_name("refresh-sweep")
        .unwrap()
        .bench(&p.clone().set("text", serde_json::Value::Null));
    assert!(refresh.is_err(), "text is not a refresh-sweep parameter");
}

#[test]
fn rate_labels() {
    assert_eq!(refresh::rate_label(144.0), "144Hz");
    assert_eq!(refresh::rate_label(59.94), "59.94Hz");
}
