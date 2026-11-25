//! Synthetic ATS schema regression data. No private save is embedded here.
//! Real ATS reference files were not supplied in the workspace.
use super::coaches::{Career, patch_xp};
use super::document::{Document, patch_money, patch_skill};
use crate::features::garages::{
    models::*, parser::parse_garages_from_sii, validator::*, writer::*,
};

pub(crate) fn fixture() -> String {
    let mut text = String::from("SiiNunit\n{\n");
    text.push_str("driver_ai : driver.other {\n experience_points: 777\n adr: 1\n heavy: 1\n}\n");
    text.push_str("economy : economy.main {\n player: player.main\n bank: bank.main\n experience_points: 43863\n adr: 3\n long_dist: 2\n heavy: 2\n fragile: 1\n urgent: 1\n mechanical: 0\n bus_experience_points: 12\n bus_total_distance: &3f000000\n garages: 3\n garages[0]: garage.las_vegas\n garages[1]: garage.los_angeles\n garages[2]: garage.phoenix\n drivers_offer: 0\n driver_pool: 1\n driver_pool[0]: driver.other\n}\n");
    text.push_str("bank : bank.main {\n money_account: 6500\n loans: 0\n}\n");
    text.push_str("player : player.main {\n driver: driver.main\n my_truck: truck.main\n assigned_vehicles: null\n hq_city: las_vegas\n trucks: 1\n trucks[0]: truck.main\n trailers: 0\n drivers: 0\n driver_flags: 0\n driver_readiness_timer: 0\n driver_undrivable_truck_timers: 0\n buses: 0\n current_bus_job: null\n}\n");
    text.push_str("driver_player : driver.main {\n bus_adr: 6\n}\n");
    text.push_str("vehicle : truck.main {\n accessories: 1\n accessories[0]: accessory.main\n odometer: 574021\n odometer_float_part: &3f000000\n fuel_relative: &3f000000\n engine_wear: &3f000000\n transmission_wear: &3f000000\n cabin_wear: &3f000000\n chassis_wear: &3f000000\n wheels_wear: 2\n wheels_wear[0]: &3f000000\n wheels_wear[1]: &3f400000\n license_plate: \"ATS|nevada\"\n}\n");
    text.push_str("vehicle_accessory : accessory.main {\n data_path: \"/def/vehicle/truck/kenworth.w900/data.sii\"\n}\n");
    for (city, slots, status) in [
        ("las_vegas", 3, 2),
        ("los_angeles", 0, 0),
        ("phoenix", 3, 2),
    ] {
        text.push_str(&format!("garage : garage.{city} {{\n vehicles: {slots}\n"));
        for index in 0..slots {
            text.push_str(&format!(" vehicles[{index}]: null\n"));
        }
        text.push_str(&format!(" drivers: {slots}\n"));
        for index in 0..slots {
            text.push_str(&format!(" drivers[{index}]: null\n"));
        }
        text.push_str(&format!(" trailers: 0\n status: {status}\n profit_log: profit.{city}\n productivity: &00000000\n future_garage_field: retained\n}}\nprofit_log : profit.{city} {{\n entries: 0\n}}\n"));
    }
    text.push_str("future_unit : future.main {\n money_account: 999\n experience_points: 888\n bus_experience_points: 999\n data: \"quoted } and { and \\\" escape\"\n future_refs: 1\n future_refs[0]: truck.main\n}\n}\n");
    text
}

#[test]
fn ats_money_xp_skills_preserve_other_units_coaches_arrays_and_crlf() {
    let original = fixture().replace('\n', "\r\n");
    let money = patch_money(&original, 9000).unwrap();
    assert_eq!(
        money,
        original.replacen("money_account: 6500", "money_account: 9000", 1)
    );
    let xp = patch_xp(&money, Career::Truck, 50000).unwrap();
    assert_eq!(
        xp,
        money.replacen("experience_points: 43863", "experience_points: 50000", 1)
    );
    let skill = patch_skill(&xp, "heavy", 6).unwrap();
    assert_eq!(skill, xp.replacen("heavy: 2", "heavy: 6", 1));
    assert!(skill.contains("bus_experience_points: 12"));
    assert!(skill.contains("bus_adr: 6"));
    assert!(patch_skill(&skill, "bus_adr", 5).is_err());
    assert!(patch_skill(&skill, "heavy", 7).is_err());
    assert!(patch_money(&skill, -1).is_err());
}

#[test]
fn ats_parser_counts_18569_units_and_roundtrips_large_save_losslessly() {
    let original = fixture();
    let count = Document::parse(&original).unwrap().units.len();
    let mut large = original[..original.len() - 2].to_string();
    for index in count..18_569 {
        large.push_str(&format!("future_unit : future.{index} {{\n value: &3f800000\n entries: 0\n text: \"unchanged }} # //\"\n a: 1\n b: 2\n c: 3\n d: 4\n e: 5\n f: 6\n g: 7\n h: 8\n}}\n"));
    }
    large.push_str("}\n");
    assert!(large.lines().count() > 240_000);
    let parsed = Document::parse(&large).unwrap();
    assert_eq!(parsed.units.len(), 18_569);
    assert_eq!(
        crate::features::truck_change::parser::parse_unit_blocks(&large).len(),
        18_569
    );
    assert_eq!(patch_money(&large, 6500).unwrap(), large);
}

#[test]
fn ats_parser_rejects_corruption_duplicate_units_and_missing_references() {
    let original = fixture();
    assert!(Document::parse(original.trim_end().trim_end_matches('}')).is_err());
    assert!(Document::parse(&original.replace("bank : bank.main", "bank : economy.main")).is_err());
    assert!(
        patch_money(
            &original.replace("bank: bank.main", "bank: bank.missing"),
            1
        )
        .is_err()
    );
    assert!(patch_skill(&original.replace(" adr: 3\n", ""), "adr", 1).is_err());
    assert!(
        patch_money(
            &original.replace(
                " money_account: 6500",
                " money_account: 6500\n money_account: 2"
            ),
            1
        )
        .is_err()
    );
}

#[test]
fn ats_garage_purchase_upgrade_and_relinquishment_verify_and_reload() {
    let original = fixture();
    assert_eq!(parse_garages_from_sii(&original).unwrap().garages.len(), 3);
    let purchased =
        apply_garage_changes(&original, "garage.los_angeles", Some((3, 5)), false).unwrap();
    let purchase_spec = GarageVerificationSpec {
        operation: GarageOperation::Purchase,
        target_size: Some(GarageSize::Large),
        set_as_headquarters: false,
    };
    verify_garage_mutation(
        &original,
        &purchased.content,
        "garage.los_angeles",
        &purchase_spec,
    )
    .unwrap();
    let upgraded =
        apply_garage_changes(&purchased.content, "garage.phoenix", Some((3, 5)), false).unwrap();
    let upgrade_spec = GarageVerificationSpec {
        operation: GarageOperation::Upgrade,
        target_size: Some(GarageSize::Large),
        set_as_headquarters: false,
    };
    verify_garage_mutation(
        &purchased.content,
        &upgraded.content,
        "garage.phoenix",
        &upgrade_spec,
    )
    .unwrap();
    let relinquished =
        apply_garage_relinquishment(&upgraded.content, "garage.los_angeles").unwrap();
    let relinquish_spec = GarageVerificationSpec {
        operation: GarageOperation::Relinquish,
        target_size: Some(GarageSize::Unowned),
        set_as_headquarters: false,
    };
    verify_garage_mutation(
        &upgraded.content,
        &relinquished.content,
        "garage.los_angeles",
        &relinquish_spec,
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!("ats-garage-roundtrip-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("game.sii");
    std::fs::write(&path, &original).unwrap();
    write_verified_content(&path, &purchased.content, |content| {
        verify_garage_mutation(&original, content, "garage.los_angeles", &purchase_spec).map(|_| ())
    })
    .unwrap();
    let reloaded = std::fs::read_to_string(&path).unwrap();
    assert_eq!(reloaded, purchased.content);
    assert_eq!(
        Document::parse(&original).unwrap().units.len(),
        Document::parse(&reloaded).unwrap().units.len()
    );
    assert!(reloaded.contains("future_garage_field: retained"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ats_ai_driver_hire_assignment_and_removal_preserve_inventory() {
    let original = fixture();
    verify_ai_driver_consistency(&original).unwrap();
    let hired = apply_ai_driver_generation(&original, "garage.phoenix", 1, false).unwrap();
    verify_ai_driver_mutation(
        &original,
        &hired.content,
        &AiDriverMutationOperation::Generate,
        &hired.affected_driver_ids,
    )
    .unwrap();
    let removed = apply_ai_driver_removal(&hired.content, &hired.affected_driver_ids).unwrap();
    verify_ai_driver_mutation(
        &hired.content,
        &removed.content,
        &AiDriverMutationOperation::Remove,
        &removed.affected_driver_ids,
    )
    .unwrap();
    assert_eq!(
        Document::parse(&original).unwrap().units.len(),
        Document::parse(&removed.content).unwrap().units.len()
    );
    assert!(removed.content.contains("bus_experience_points: 12"));
}

#[test]
fn ats_atomic_write_rejects_hash_changes_invalid_data_and_verification_failure() {
    let root = std::env::temp_dir().join(format!("ats-atomic-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("game.sii");
    let original = fixture();
    std::fs::write(&path, &original).unwrap();
    let edited = patch_money(&original, 7000).unwrap();
    assert!(
        super::safety::write_atomic_checked(&path, b"stale hash", &edited, |_| Ok(())).is_err()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert!(
        super::safety::write_atomic_checked(&path, original.as_bytes(), "SiiNunit {", |_| Ok(()))
            .is_err()
    );
    assert!(
        super::safety::write_atomic_checked(&path, original.as_bytes(), &edited, |_| Err(
            "verification_failure".into()
        ))
        .is_err()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    super::safety::write_atomic_checked(&path, original.as_bytes(), &edited, |_| Ok(())).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ats_cannot_select_or_write_ets2_profile_and_save() {
    use crate::shared::game::GameType;
    let root = std::env::temp_dir().join(format!("ats-isolation-{}", uuid::Uuid::new_v4()));
    let ets2 = root.join("Euro Truck Simulator 2/profiles/p");
    let ats = root.join("American Truck Simulator/profiles/p");
    let ats_save = ats.join("save/1");
    let ets_save = ets2.join("save/1");
    for save in [&ats_save, &ets_save] {
        std::fs::create_dir_all(save).unwrap();
        std::fs::write(save.join("game.sii"), fixture()).unwrap();
    }
    assert!(super::safety::validate_profile_game(&ets2, GameType::Ats).is_err());
    assert!(super::safety::validate_profile_game(&ats, GameType::Ets2).is_err());
    assert!(super::safety::resolve_save(&ats, &ets_save).is_err());
    let state = crate::state::AppProfileState::default();
    *state.selected_game.lock().unwrap() = "ats".into();
    *state.current_profile.lock().unwrap() = Some(ats.display().to_string());
    *state.current_save.lock().unwrap() = Some(ats_save.display().to_string());
    let game = super::safety::active_save(&state).unwrap();
    super::safety::remember_save(&state, &game).unwrap();
    std::fs::write(&game, patch_money(&fixture(), 1).unwrap()).unwrap();
    assert!(
        super::safety::check_loaded_hash(&state, &game, &std::fs::read(&game).unwrap()).is_err()
    );
    assert_eq!(
        std::fs::read_to_string(ets_save.join("game.sii")).unwrap(),
        fixture()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ets2_skill_regression_edits_economy_preserves_driver_and_bus_fields() {
    let original = include_str!("../../../test-fixtures/decrypt/plain_game.sii");
    let doc = Document::parse(original).unwrap();
    let economy = doc.unique("economy").unwrap();
    assert_eq!(doc.field(economy, "adr").unwrap(), "63");
    let target = doc.field_span(economy, "adr").unwrap();
    let mut expected = original.to_string();
    expected.replace_range(target, "31");
    assert_eq!(patch_skill(original, "adr", 31).unwrap(), expected);
}

#[test]
fn ats_layout_gate_requires_known_statuses_capacities_and_complete_references() {
    use crate::features::garages::service::validate_write_layout;
    let original = fixture();
    validate_write_layout(&original, "ats").unwrap();
    assert!(validate_write_layout(&original.replace(" status: 0", " status: 7"), "ats").is_err());
    assert!(
        validate_write_layout(
            &original.replace(" profit_log: profit.phoenix", " profit_log: profit.missing"),
            "ats"
        )
        .is_err()
    );
    assert!(
        validate_write_layout(
            &original.replace(" vehicles[1]: null", " vehicles[1]: truck.missing"),
            "ats"
        )
        .is_err()
    );
}

#[test]
fn ats_save_pair_rolls_back_original_bytes_on_metadata_write_failure() {
    let root = std::env::temp_dir().join(format!("ats-rollback-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let game = root.join("game.sii");
    let info = root.join("info.sii");
    let before = fixture().replace('\n', "\r\n");
    let metadata = "SiiNunit\n{\nsave_container : save.main {\n info_money_account: 1500\n}\n}\n";
    std::fs::write(&game, &before).unwrap();
    std::fs::write(&info, metadata).unwrap();
    let backup = crate::features::backup::service::recommended_targets(&game);
    assert!(backup.contains(&game) && backup.contains(&info));
    let updated = patch_money(&before, 9999).unwrap();
    let failure = super::safety::write_save_pair(
        &game,
        before.as_bytes(),
        &updated,
        &info,
        metadata.as_bytes(),
        metadata,
        |_| Err("injected_metadata_write_failure".into()),
    )
    .unwrap_err();
    assert_eq!(failure, "injected_metadata_write_failure");
    assert_eq!(std::fs::read(&game).unwrap(), before.as_bytes());
    assert_eq!(std::fs::read_to_string(&info).unwrap(), metadata);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn ats_backup_restore_is_atomic_and_rolls_back_on_a_changed_target() {
    let root = std::env::temp_dir().join(format!("ats-restore-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let game = root.join("game.sii");
    let info = root.join("info.sii");
    std::fs::write(&game, b"original-game").unwrap();
    std::fs::write(&info, b"external-info").unwrap();
    let replacements = vec![
        (
            game.clone(),
            Some(b"original-game".to_vec()),
            b"backup-game".to_vec(),
        ),
        (
            info.clone(),
            Some(b"stale-info".to_vec()),
            b"backup-info".to_vec(),
        ),
    ];
    let error =
        crate::features::backup::service::restore_files_atomically(&replacements).unwrap_err();
    assert_eq!(error, "save_changed_since_load");
    assert_eq!(std::fs::read(&game).unwrap(), b"original-game");
    assert_eq!(std::fs::read(&info).unwrap(), b"external-info");
    let replacements = vec![
        (
            game.clone(),
            Some(b"original-game".to_vec()),
            b"backup-game".to_vec(),
        ),
        (
            info.clone(),
            Some(b"external-info".to_vec()),
            b"backup-info".to_vec(),
        ),
    ];
    crate::features::backup::service::restore_files_atomically(&replacements).unwrap();
    assert_eq!(std::fs::read(&game).unwrap(), b"backup-game");
    assert_eq!(std::fs::read(&info).unwrap(), b"backup-info");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    std::fs::remove_dir_all(root).unwrap();
}
