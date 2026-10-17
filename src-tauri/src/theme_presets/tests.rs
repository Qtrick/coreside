//! Theme preset repository + atomic apply integration tests.

use super::*;
use crate::db::{self, Database};
use tempfile::tempdir;

fn open_db() -> (tempfile::TempDir, Database) {
    let dir = tempdir().unwrap();
    let db = Database::open_path(&dir.path().join("theme.db")).unwrap();
    (dir, db)
}

fn seed_appearance(db: &mut Database, theme: &str, wallpaper_kind: &str, transparency: u8) {
    db::set_setting(db, "theme", theme).unwrap();
    db::set_setting(db, "accentPrimaryLight", "#112233").unwrap();
    db::set_setting(db, "accentPrimaryDark", "#aabbcc").unwrap();
    db::set_setting(db, "accentSecondaryLight", "#223344").unwrap();
    db::set_setting(db, "accentSecondaryDark", "#bbccdd").unwrap();
    db::set_setting(db, "backgroundLight", "#f0f0f0").unwrap();
    db::set_setting(db, "backgroundDark", "#101010").unwrap();
    db::set_setting(db, "surfaceLight", "#ffffff").unwrap();
    db::set_setting(db, "surfaceDark", "#202020").unwrap();
    db::set_setting(db, "surfaceMutedLight", "#eeeeee").unwrap();
    db::set_setting(db, "surfaceMutedDark", "#303030").unwrap();
    db::set_setting(db, "borderLight", "#dddddd").unwrap();
    db::set_setting(db, "borderDark", "#404040").unwrap();
    db::set_setting(db, "textPrimaryLight", "#111111").unwrap();
    db::set_setting(db, "textPrimaryDark", "#f5f5f5").unwrap();
    db::set_setting(db, "textSecondaryLight", "#666666").unwrap();
    db::set_setting(db, "textSecondaryDark", "#aaaaaa").unwrap();
    db::set_setting(
        db,
        "wallpaper",
        &format!(r#"{{"kind":"{wallpaper_kind}"}}"#),
    )
    .unwrap();
    db::set_setting(db, "wallpaperJson", "").unwrap();
    db::set_setting(db, "interfaceTransparency", &transparency.to_string()).unwrap();
    // Unrelated setting must survive apply.
    db::set_setting(db, "preferredModel", "auto").unwrap();
}

#[test]
fn save_captures_full_appearance_and_survives_reopen() {
    let (dir, mut db) = open_db();
    seed_appearance(&mut db, "dark", "matrix", 42);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    assert_eq!(snap.theme_preference, "dark");
    assert_eq!(snap.wallpaper.kind, "matrix");
    assert_eq!(snap.interface_transparency, 42);
    assert_eq!(snap.accent_primary_light, "#112233");
    assert_eq!(snap.accent_primary_dark, "#aabbcc");
    assert_eq!(snap.border_light, "#dddddd");
    assert_eq!(snap.border_dark, "#404040");

    let record = insert_theme_preset(&mut db, "Midnight Matrix", "", &snap, 10).unwrap();
    assert_eq!(record.name, "Midnight Matrix");
    drop(db);

    let db2 = Database::open_path(&dir.path().join("theme.db")).unwrap();
    let listed = list_theme_presets(&db2).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].snapshot.wallpaper.kind, "matrix");
    assert_eq!(listed[0].snapshot.interface_transparency, 42);
}

#[test]
fn free_quota_rejects_eleventh_and_allows_update_at_limit() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "system", "none", 0);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    for i in 0..10 {
        insert_theme_preset(&mut db, &format!("Theme {i}"), "", &snap, 10).unwrap();
    }
    let err = insert_theme_preset(&mut db, "Theme 10", "", &snap, 10).unwrap_err();
    assert!(err.to_string().contains("limit"));

    let first = list_theme_presets(&db).unwrap().into_iter().next().unwrap();
    seed_appearance(&mut db, "light", "aurora", 10);
    let new_snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    let updated =
        update_theme_preset_snapshot(&mut db, &first.id, &new_snap, first.revision).unwrap();
    assert_eq!(updated.snapshot.wallpaper.kind, "aurora");
    assert_eq!(count_theme_presets(&db).unwrap(), 10);
}

#[test]
fn apply_is_atomic_and_preserves_unrelated_settings() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "dark", "matrix", 55);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    let preset = insert_theme_preset(&mut db, "Keep Me", "", &snap, 10).unwrap();

    seed_appearance(&mut db, "light", "none", 0);
    db::set_setting(&mut db, "preferredModel", "kept-model").unwrap();

    apply_appearance_snapshot(&mut db, &preset.snapshot).unwrap();
    let map = db::get_settings(&db).unwrap();
    assert_eq!(map.get("theme").map(String::as_str), Some("dark"));
    assert!(map.get("wallpaper").unwrap().contains("matrix"));
    assert_eq!(
        map.get("interfaceTransparency").map(String::as_str),
        Some("55")
    );
    assert_eq!(
        map.get("accentPrimaryLight").map(String::as_str),
        Some("#112233")
    );
    assert_eq!(
        map.get("preferredModel").map(String::as_str),
        Some("kept-model")
    );
}

#[test]
fn apply_restores_every_snapshot_field_and_preserves_project_wallpaper() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "dark", "matrix", 55);
    let mut snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    snap.wallpaper_json =
        r##"{"schemaVersion":"1","type":"static-color","color":"#123456"}"##.into();
    snap.wallpaper.kind = "aurora".into();
    let preset = insert_theme_preset(&mut db, "Exact", "", &snap, 10).unwrap();

    db.conn()
        .execute(
            "INSERT INTO projects (id, name, wallpaper_json) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                "project-themed",
                "Themed project",
                r##"{"schemaVersion":"1","type":"static-color","color":"#654321"}"##
            ],
        )
        .unwrap();
    seed_appearance(&mut db, "light", "none", 0);

    apply_appearance_snapshot(&mut db, &preset.snapshot).unwrap();

    let map = db::get_settings(&db).unwrap();
    let expected = [
        ("theme", "dark"),
        ("accentPrimaryLight", "#112233"),
        ("accentPrimaryDark", "#aabbcc"),
        ("accentSecondaryLight", "#223344"),
        ("accentSecondaryDark", "#bbccdd"),
        ("backgroundLight", "#f0f0f0"),
        ("backgroundDark", "#101010"),
        ("surfaceLight", "#ffffff"),
        ("surfaceDark", "#202020"),
        ("surfaceMutedLight", "#eeeeee"),
        ("surfaceMutedDark", "#303030"),
        ("borderLight", "#dddddd"),
        ("borderDark", "#404040"),
        ("textPrimaryLight", "#111111"),
        ("textPrimaryDark", "#f5f5f5"),
        ("textSecondaryLight", "#666666"),
        ("textSecondaryDark", "#aaaaaa"),
        (
            "wallpaperJson",
            r##"{"schemaVersion":"1","type":"static-color","color":"#123456"}"##,
        ),
        ("wallpaper", r#"{"kind":"none"}"#),
        ("interfaceTransparency", "55"),
    ];
    for (key, value) in expected {
        assert_eq!(map.get(key).map(String::as_str), Some(value), "{key}");
    }
    let project_wallpaper: String = db
        .conn()
        .query_row(
            "SELECT wallpaper_json FROM projects WHERE id = 'project-themed'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(project_wallpaper.contains("#654321"));
}

#[test]
fn apply_rolls_back_all_writes_when_a_later_setting_write_fails() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "dark", "matrix", 55);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();

    seed_appearance(&mut db, "light", "none", 0);
    let before = db::get_settings(&db).unwrap();
    db.conn()
        .execute_batch(
            "CREATE TRIGGER reject_surface_dark_insert
             BEFORE INSERT ON settings
             WHEN NEW.key = 'surfaceDark'
             BEGIN
               SELECT RAISE(ABORT, 'forced apply failure');
             END;",
        )
        .unwrap();

    let result = apply_appearance_snapshot(&mut db, &snap);

    assert!(result.is_err());
    assert_eq!(db::get_settings(&db).unwrap(), before);
}

#[test]
fn delete_does_not_change_active_appearance() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "dark", "matrix", 30);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    let preset = insert_theme_preset(&mut db, "Temp", "", &snap, 10).unwrap();
    delete_theme_preset(&mut db, &preset.id, preset.revision).unwrap();
    let map = db::get_settings(&db).unwrap();
    assert_eq!(map.get("theme").map(String::as_str), Some("dark"));
    assert!(map.get("wallpaper").unwrap().contains("matrix"));
}

#[test]
fn rename_preserves_snapshot_stale_revision_rejected() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "system", "particles", 5);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    let preset = insert_theme_preset(&mut db, "Old", "desc", &snap, 10).unwrap();
    let renamed = rename_theme_preset(&mut db, &preset.id, "New", preset.revision).unwrap();
    assert_eq!(renamed.name, "New");
    assert_eq!(renamed.snapshot, snap);
    let err = rename_theme_preset(&mut db, &preset.id, "Again", preset.revision).unwrap_err();
    assert!(matches!(err, db::DbError::Conflict(_)));
}

#[test]
fn stale_delete_is_rejected_without_removing_the_newer_preset() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "system", "particles", 5);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    let preset = insert_theme_preset(&mut db, "Original", "", &snap, 10).unwrap();
    let renamed = rename_theme_preset(&mut db, &preset.id, "Renamed", preset.revision).unwrap();

    let err = delete_theme_preset(&mut db, &preset.id, preset.revision).unwrap_err();

    assert!(matches!(err, db::DbError::Conflict(_)));
    assert_eq!(get_theme_preset(&db, &preset.id).unwrap(), renamed);
}

#[test]
fn corrupt_preset_makes_list_and_get_fail_closed() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "system", "none", 0);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    let preset = insert_theme_preset(&mut db, "Corrupt", "", &snap, 10).unwrap();
    db.conn()
        .execute(
            "UPDATE theme_presets SET snapshot_json = '{}' WHERE id = ?1",
            [&preset.id],
        )
        .unwrap();

    assert!(matches!(
        list_theme_presets(&db),
        Err(db::DbError::Corrupted(_))
    ));
    assert!(matches!(
        get_theme_preset(&db, &preset.id),
        Err(db::DbError::Corrupted(_))
    ));
}

#[test]
fn schema_version_mismatch_makes_list_and_get_fail_closed() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "system", "none", 0);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    let preset = insert_theme_preset(&mut db, "Mismatched", "", &snap, 10).unwrap();
    db.conn()
        .execute(
            "UPDATE theme_presets SET schema_version = 2 WHERE id = ?1",
            [&preset.id],
        )
        .unwrap();

    assert!(matches!(
        list_theme_presets(&db),
        Err(db::DbError::Corrupted(_))
    ));
    assert!(matches!(
        get_theme_preset(&db, &preset.id),
        Err(db::DbError::Corrupted(_))
    ));
}

#[test]
fn missing_media_rejects_apply() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "dark", "none", 0);
    let mut snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    snap.wallpaper_json =
        r#"{"schemaVersion":"1","type":"image-cover","assetId":"missing-asset","opacity":0.5}"#
            .into();
    // Validation of wallpaperJson may fail at parse if schema invalid — expect either invalid or not found.
    let result = apply_appearance_snapshot(&mut db, &snap);
    assert!(result.is_err());
    // Active appearance unchanged.
    let map = db::get_settings(&db).unwrap();
    assert_eq!(map.get("theme").map(String::as_str), Some("dark"));
}

#[test]
fn downgrade_preserves_presets_over_free_limit() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "system", "none", 0);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    for i in 0..15 {
        insert_theme_preset(&mut db, &format!("Pro {i}"), "", &snap, 50).unwrap();
    }
    assert_eq!(count_theme_presets(&db).unwrap(), 15);
    // Free limit blocks new creates but does not prune.
    let err = insert_theme_preset(&mut db, "Extra", "", &snap, 10).unwrap_err();
    assert!(err.to_string().contains("limit"));
    assert_eq!(count_theme_presets(&db).unwrap(), 15);
}

#[test]
fn duplicate_names_conflict() {
    let (_dir, mut db) = open_db();
    seed_appearance(&mut db, "light", "none", 0);
    let snap = AppearanceSnapshot::from_settings_map(&db::get_settings(&db).unwrap()).unwrap();
    insert_theme_preset(&mut db, "Same", "", &snap, 10).unwrap();
    let err = insert_theme_preset(&mut db, "same", "", &snap, 10).unwrap_err();
    assert!(matches!(err, db::DbError::Conflict(_)));
}
