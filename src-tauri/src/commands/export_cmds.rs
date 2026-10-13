//! Export IPC — writes only after the user chooses a path.

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use tauri::State;

use super::CommandError;
use crate::db;
use crate::exports::{
    assert_no_secrets, coreside_tool_package, resolve_managed_export_path, sanitize_filename,
    standalone_html_for_clock, strip_secrets_from_tool,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportToolInput {
    pub tool_id: String,
    pub format: String,
    pub destination_path: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub ok: bool,
    pub path: String,
    pub format: String,
    pub message: String,
}

fn clock_props(def: &serde_json::Value) -> serde_json::Value {
    def.get("components")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter().find(|c| {
                c.get("type")
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| t == "clock")
            })
        })
        .and_then(|c| c.get("props").cloned())
        .unwrap_or_else(|| serde_json::json!({}))
}

#[tauri::command]
pub fn export_tool(
    state: State<'_, AppState>,
    input: ExportToolInput,
) -> Result<ExportResult, CommandError> {
    state.require_profile()?;
    let format = input.format.trim().to_lowercase();

    let (tool_json, tool_name, state_json) = {
        let db = state.db.lock();
        let tool = db::get_tool(&db, input.tool_id.trim())?;
        let mut def = tool.definition;
        def.normalize_for_frontend();
        let tool_json = serde_json::to_value(&def)
            .map_err(|e| CommandError::new("serialize", e.to_string()))?;
        let state_json = db::get_tool_state(&db, &tool.id)?.unwrap_or(serde_json::json!({}));
        (tool_json, def.name, state_json)
    };

    let cleaned = strip_secrets_from_tool(&tool_json);
    let cleaned_state = strip_secrets_from_tool(&state_json);
    let base = sanitize_filename(&tool_name);

    let (bytes, default_name) = match format.as_str() {
        "coreside-tool" | "json" => {
            let package =
                coreside_tool_package(&cleaned, Some(&cleaned_state), env!("CARGO_PKG_VERSION"));
            let text = serde_json::to_string_pretty(&package)
                .map_err(|e| CommandError::new("serialize", e.to_string()))?;
            assert_no_secrets(&text).map_err(|e| CommandError::new("security", e))?;
            (text.into_bytes(), format!("{base}.coreside-tool.json"))
        }
        "html" => {
            let props = clock_props(&cleaned);
            let html = standalone_html_for_clock(&tool_name, &props);
            assert_no_secrets(&html).map_err(|e| CommandError::new("security", e))?;
            (html.into_bytes(), format!("{base}.html"))
        }
        "zip" => {
            let props = clock_props(&cleaned);
            let html = standalone_html_for_clock(&tool_name, &props);
            assert_no_secrets(&html).map_err(|e| CommandError::new("security", e))?;
            let readme = format!(
                "Coreside export of {tool_name}\nOffline HTML runtime included.\nSecrets are never included.\n"
            );
            let bundle = format!(
                "CORESIDE-ZIP-V1\n--- index.html ---\n{html}\n--- README.txt ---\n{readme}\n--- manifest.json ---\n{{\"format\":\"coreside-web-bundle\",\"tool\":{}}}\n",
                cleaned
            );
            assert_no_secrets(&bundle).map_err(|e| CommandError::new("security", e))?;
            (bundle.into_bytes(), format!("{base}.zip.txt"))
        }
        other => {
            return Err(CommandError::new(
                "unsupported",
                format!("Export format '{other}' is not available for this tool"),
            ));
        }
    };

    let requested = input.destination_path.trim();
    let dest_name = if requested.is_empty() {
        default_name.clone()
    } else {
        let raw = PathBuf::from(requested);
        let base = raw
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(requested);
        if PathBuf::from(base).extension().is_some() {
            base.to_string()
        } else if let Some(ext) = PathBuf::from(&default_name)
            .extension()
            .and_then(|e| e.to_str())
        {
            format!("{base}.{ext}")
        } else {
            base.to_string()
        }
    };
    let final_path = resolve_managed_export_path(&dest_name, &default_name)
        .map_err(|e| CommandError::new("invalid", e))?;

    fs::write(&final_path, &bytes)
        .map_err(|e| CommandError::new("io", format!("Could not write export: {e}")))?;
    if !final_path.exists() {
        return Err(CommandError::new("io", "Export file was not created"));
    }

    {
        let db = state.db.lock();
        let _ = db.conn().execute(
            "INSERT INTO export_history (id, tool_id, format, filename, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'ok', datetime('now'))",
            rusqlite::params![
                format!("exp-{}", uuid::Uuid::new_v4()),
                input.tool_id,
                format,
                final_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("export"),
            ],
        );
    }

    Ok(ExportResult {
        ok: true,
        path: final_path.to_string_lossy().into_owned(),
        format,
        message: "Export saved".into(),
    })
}

#[tauri::command]
pub fn list_export_formats(
    tool_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, CommandError> {
    state.require_profile()?;
    let db = state.db.lock();
    let tool = db::get_tool(&db, tool_id.trim())?;
    let mut def = tool.definition;
    def.normalize_for_frontend();
    let value = serde_json::to_value(&def).unwrap_or_default();
    let has_clock = value
        .get("components")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter().any(|c| {
                c.get("type")
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| t == "clock")
            })
        })
        .unwrap_or(false);

    let mut formats = vec![
        serde_json::json!({
            "id": "coreside-tool",
            "label": "Coreside Tool Package",
            "extension": "coreside-tool.json",
            "available": true,
            "description": "Definition and safe state for backup or re-import."
        }),
        // No PNG entry: Coreside has no real DOM-to-image capture, and the
        // previous implementation produced a blank placeholder image while
        // reporting success.
    ];
    formats.push(serde_json::json!({
        "id": "html",
        "label": "Standalone HTML",
        "extension": "html",
        "available": has_clock,
        "description": if has_clock {
            "Self-contained HTML that keeps updating offline."
        } else {
            "Available for tools with a trusted clock component."
        }
    }));
    formats.push(serde_json::json!({
        "id": "zip",
        "label": "ZIP Web Bundle",
        "extension": "zip.txt",
        "available": has_clock,
        "description": if has_clock {
            "Offline bundle with HTML runtime and README."
        } else {
            "Requires a compatible trusted component."
        }
    }));
    Ok(formats)
}
