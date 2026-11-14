use super::coaches::{Career, patch_xp, read_snapshot, save_hash, verify_xp};

fn fixture() -> String {
    "SiiNunit\n{\n\
driver_ai : driver.other {\n experience_points: 777\n}\n\
economy : economy.main {\n player: player.main\n experience_points: 43863\n bus_experience_points: 0\n\
 bus_total_distance: 42\n bus_finished_job_count: 3\n bus_cancelled_job_count: 1\n\
 bus_total_passengers: 25\n bus_total_stops: 9\n bus_game_time: 123\n bus_playing_time: 456\n\
 bus_stops: 3\n bus_stops[0]: bus_stop.one\n bus_stops[1]: bus_stop.two\n bus_stops[2]: bus_stop.three\n\
 future_field: &ff008822\n future_refs: 1\n future_refs[0]: future.unit\n}\n\
player : player.main {\n my_vehicles_mode: truck\n assigned_vehicles_mode: truck\n buses: 1\n buses[0]: vehicle.bus\n current_bus_job: job.bus\n skills: 63\n}\n\
bus_stop : bus_stop.one {\n discovered: true\n lines_offer: 0\n}\n\
bus_stop : bus_stop.two {\n discovered: false\n lines_offer: 0\n}\n\
bus_stop : bus_stop.three {\n discovered: true\n lines_offer: 0\n}\n\
vehicle : vehicle.bus {\n unknown_bus_field: keep_me\n}\n\
unknown_job : job.bus {\n route: future.unit\n}\n\
bus_job_log : log.bus {\n version: 0\n entries: 0\n}\n\
future_unit : future.unit {\n experience_points: 888\n bus_experience_points: 999\n data: \"unchanged value\"\n}\n}\n".to_string()
}

#[test]
fn reads_both_careers_from_economy_only() {
    let snapshot = read_snapshot(&fixture()).unwrap();
    assert_eq!(snapshot.truck_xp, Some(43863));
    assert_eq!(snapshot.bus_xp, Some(0));
    assert_eq!(snapshot.statistics["bus_game_time"].as_deref(), Some("123"));
    assert_eq!(
        snapshot.statistics["bus_playing_time"].as_deref(),
        Some("456")
    );
}

#[test]
fn truck_edit_changes_only_the_economy_field() {
    let original = fixture();
    let changed = patch_xp(&original, Career::Truck, 50000).unwrap();
    assert_eq!(
        changed,
        original.replacen(" experience_points: 43863", " experience_points: 50000", 1)
    );
    assert_eq!(read_snapshot(&changed).unwrap().bus_xp, Some(0));
}

#[test]
fn bus_edit_changes_only_the_economy_field() {
    let original = fixture();
    let changed = patch_xp(&original, Career::Bus, 1234).unwrap();
    assert_eq!(
        changed,
        original.replacen(
            " bus_experience_points: 0",
            " bus_experience_points: 1234",
            1
        )
    );
    assert_eq!(read_snapshot(&changed).unwrap().truck_xp, Some(43863));
}

#[test]
fn independent_edits_survive_disk_roundtrip() {
    let updated = patch_xp(&fixture(), Career::Truck, 50000).unwrap();
    let updated = patch_xp(&updated, Career::Bus, 5678).unwrap();
    let directory =
        std::env::temp_dir().join(format!("ets2-coaches-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("game.sii");
    std::fs::write(&path, &updated).unwrap();
    let reloaded = std::fs::read_to_string(path).unwrap();
    verify_xp(&reloaded, Career::Truck, 50000).unwrap();
    verify_xp(&reloaded, Career::Bus, 5678).unwrap();
    assert_eq!(updated, reloaded);
    // The fixture is synthetic; no original game save is opened or modified.
}

#[test]
fn legacy_save_keeps_truck_editing_without_adding_bus_fields() {
    let original = "SiiNunit\n{\neconomy : old.save {\n experience_points: 25\n}\n}\n";
    let snapshot = read_snapshot(original).unwrap();
    assert_eq!(snapshot.bus_xp, None);
    assert!(snapshot.stops.is_empty());
    assert_eq!(snapshot.buses, None);
    assert!(patch_xp(original, Career::Bus, 10).is_err());
    let updated = patch_xp(original, Career::Truck, 42).unwrap();
    assert!(!updated.contains("bus_"));
    verify_xp(&updated, Career::Truck, 42).unwrap();
}

#[test]
fn stops_are_counted_from_references_and_discovery_values() {
    let snapshot = read_snapshot(&fixture()).unwrap();
    assert_eq!(snapshot.stops.len(), 3);
    assert_eq!(snapshot.discovered_stops, 2);
    assert_eq!(snapshot.stops[1].discovered, Some(false));
    assert!(snapshot.warnings.is_empty());
}

#[test]
fn counts_large_stop_arrays_without_fixed_totals() {
    for (total, discovered) in [(131, 22), (7, 3), (0, 0)] {
        let mut content = format!(
            "SiiNunit\n{{\neconomy : economy.main {{\n experience_points: 1\n bus_stops: {total}\n"
        );
        for index in 0..total {
            content.push_str(&format!(" bus_stops[{index}]: bus_stop.test_{index}\n"));
        }
        content.push_str("}\n");
        for index in 0..total {
            content.push_str(&format!(
                "bus_stop : bus_stop.test_{index} {{\n discovered: {}\n lines_offer: 0\n}}\n",
                index < discovered
            ));
        }
        content.push_str("}\n");
        let snapshot = read_snapshot(&content).unwrap();
        assert_eq!(snapshot.stops.len(), total);
        assert_eq!(snapshot.discovered_stops, discovered);
        assert!(snapshot.warnings.is_empty());
    }
}

#[test]
fn unresolved_stop_is_not_counted_as_discovered() {
    let content = fixture().replace(
        "bus_stops[0]: bus_stop.one",
        "bus_stops[0]: bus_stop.missing",
    );
    let snapshot = read_snapshot(&content).unwrap();
    assert_eq!(snapshot.discovered_stops, 1);
    assert!(!snapshot.stops[0].resolved);
    assert_eq!(snapshot.stops[0].discovered, None);
    assert!(!snapshot.warnings.is_empty());
}

#[test]
fn stop_counts_are_dynamic_and_ignore_unreferenced_units() {
    let mut content = fixture().replace("bus_stops: 3", "bus_stops: 131");
    content = content.replace(
        " bus_stops[2]: bus_stop.three\n",
        " bus_stops[2]: bus_stop.three\n bus_stops[3]: bus_stop.four\n",
    );
    let snapshot = read_snapshot(&content).unwrap();
    assert_eq!(snapshot.stops.len(), 4);
    assert_eq!(snapshot.discovered_stops, 2);
    assert!(
        snapshot
            .warnings
            .iter()
            .any(|warning| warning == "coaches.stop_array_invalid")
    );
}

#[test]
fn bus_objects_and_log_are_detected_without_assuming_their_schema() {
    let snapshot = read_snapshot(&fixture()).unwrap();
    assert_eq!(snapshot.buses.as_deref(), Some("1"));
    assert_eq!(snapshot.bus_references.len(), 1);
    assert!(snapshot.bus_references[0].resolved);
    assert_eq!(snapshot.current_bus_job.as_deref(), Some("job.bus"));
    assert!(snapshot.current_bus_job_resolved);
    assert_eq!(snapshot.job_logs[0].version.as_deref(), Some("0"));
    assert_eq!(snapshot.job_logs[0].entries.as_deref(), Some("0"));
}

#[test]
fn zero_buses_and_null_job_remain_distinct_from_missing_fields() {
    let content = fixture()
        .replace(" buses: 1\n buses[0]: vehicle.bus", " buses: 0")
        .replace("current_bus_job: job.bus", "current_bus_job: null");
    let snapshot = read_snapshot(&content).unwrap();
    assert_eq!(snapshot.buses.as_deref(), Some("0"));
    assert!(snapshot.bus_references.is_empty());
    assert_eq!(snapshot.current_bus_job.as_deref(), Some("null"));
    assert!(!snapshot.current_bus_job_resolved);
}

#[test]
fn negative_and_unsafe_xp_are_rejected() {
    for value in [-1, 9_007_199_254_740_992, i64::MAX] {
        assert!(patch_xp(&fixture(), Career::Bus, value).is_err());
    }
    assert!(patch_xp(&fixture(), Career::Bus, 0).is_ok());
}

#[test]
fn missing_invalid_or_ambiguous_xp_are_never_repaired_automatically() {
    for value in ["-1", "unknown", "9223372036854775808"] {
        let content = fixture().replacen(
            "bus_experience_points: 0",
            &format!("bus_experience_points: {value}"),
            1,
        );
        assert_eq!(read_snapshot(&content).unwrap().bus_xp, None);
        assert!(patch_xp(&content, Career::Bus, 10).is_err());
    }
    let duplicated = fixture().replacen(
        "bus_experience_points: 0",
        "bus_experience_points: 0\n bus_experience_points: 1",
        1,
    );
    assert!(patch_xp(&duplicated, Career::Bus, 10).is_err());
    let ambiguous = format!(
        "{}\neconomy : extra {{\n experience_points: 1\n}}\n",
        fixture()
    );
    assert!(patch_xp(&ambiguous, Career::Truck, 10).is_err());
}

#[test]
fn crlf_and_unknown_fields_and_references_survive() {
    let original = fixture().replace('\n', "\r\n");
    let updated = patch_xp(&original, Career::Bus, 99).unwrap();
    assert_eq!(
        updated,
        original.replacen(" bus_experience_points: 0", " bus_experience_points: 99", 1)
    );
}

#[test]
fn stale_content_has_a_different_hash_and_wrong_readback_fails() {
    let original = fixture();
    let updated = patch_xp(&original, Career::Bus, 99).unwrap();
    assert_ne!(save_hash(&original), save_hash(&updated));
    assert!(verify_xp(&updated, Career::Bus, 100).is_err());
}

#[test]
fn career_type_only_accepts_explicit_supported_careers() {
    assert_eq!(
        serde_json::from_str::<Career>("\"truck\"").unwrap(),
        Career::Truck
    );
    assert_eq!(
        serde_json::from_str::<Career>("\"bus\"").unwrap(),
        Career::Bus
    );
    for input in ["\"coach\"", "\"Truck\"", "\"all\"", "null", "0"] {
        assert!(serde_json::from_str::<Career>(input).is_err());
    }
    assert_eq!(Career::Truck.xp_field(), "experience_points");
    assert_eq!(Career::Bus.xp_field(), "bus_experience_points");
}

#[test]
fn saving_bus_then_truck_keeps_both_careers_and_other_units_independent() {
    let original = fixture();
    let bus_saved = patch_xp(&original, Career::Bus, 1234).unwrap();
    let both_saved = patch_xp(&bus_saved, Career::Truck, 50000).unwrap();
    let expected = original
        .replacen(
            " bus_experience_points: 0",
            " bus_experience_points: 1234",
            1,
        )
        .replacen(" experience_points: 43863", " experience_points: 50000", 1);
    assert_eq!(both_saved, expected);
    let snapshot = read_snapshot(&both_saved).unwrap();
    assert_eq!(snapshot.truck_xp, Some(50000));
    assert_eq!(snapshot.bus_xp, Some(1234));
    assert_eq!(snapshot.discovered_stops, 2);
}
