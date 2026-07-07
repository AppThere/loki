// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document settings in the Loro CRDT.
//!
//! [`DocumentSettings`] (default tab stop, **track changes**, …) is stored as a
//! lossless `serde` JSON snapshot under [`KEY_SETTINGS_JSON`] in the settings
//! map — the same strategy as metadata and comments — so a settings change made
//! in the editor survives incremental rebuilds, undo/redo, and Loro
//! import/export. `Document::settings` is `Option`; an absent map reads back as
//! `None`.

use loro::{LoroDoc, LoroMap};

use super::BridgeError;
use crate::loro_schema::{KEY_SETTINGS, KEY_SETTINGS_JSON};
use crate::settings::DocumentSettings;

/// Writes `settings` into the settings `map` as a JSON snapshot (a no-op when
/// `None`, leaving the map empty so it reads back as `None`).
pub(super) fn write_settings(
    settings: Option<&DocumentSettings>,
    map: &LoroMap,
) -> Result<(), BridgeError> {
    if let Some(settings) = settings {
        write_settings_json(settings, map);
    }
    Ok(())
}

#[cfg(feature = "serde")]
fn write_settings_json(settings: &DocumentSettings, map: &LoroMap) {
    match serde_json::to_string(settings) {
        Ok(json) => {
            if let Err(err) = map.insert(KEY_SETTINGS_JSON, json) {
                tracing::warn!("loro bridge: failed to store settings snapshot: {err}");
            }
        }
        // Unreachable in practice: DocumentSettings derives Serialize.
        Err(err) => tracing::warn!("loro bridge: failed to serialize settings: {err}"),
    }
}

#[cfg(not(feature = "serde"))]
fn write_settings_json(_settings: &DocumentSettings, _map: &LoroMap) {
    tracing::warn!(
        "loro bridge: settings not persisted — build loki-doc-model with the \
         `serde` feature (default) to round-trip document settings"
    );
}

/// Reads the document settings from the settings `map`. `None` when absent.
pub(super) fn read_settings(map: &LoroMap) -> Option<DocumentSettings> {
    read_settings_json(map)
}

#[cfg(feature = "serde")]
fn read_settings_json(map: &LoroMap) -> Option<DocumentSettings> {
    let json = map
        .get(KEY_SETTINGS_JSON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())?;
    serde_json::from_str(&json).ok()
}

#[cfg(not(feature = "serde"))]
fn read_settings_json(_map: &LoroMap) -> Option<DocumentSettings> {
    None
}

/// Sets the document's **track changes** flag in `loro` (Review tab, 4a.2),
/// preserving the other settings. Undoable; the caller commits + relayouts.
pub fn set_track_changes(loro: &LoroDoc, on: bool) -> Result<(), BridgeError> {
    let map = loro.get_map(KEY_SETTINGS);
    let mut settings = read_settings(&map).unwrap_or_default();
    settings.track_changes = on;
    write_settings(Some(&settings), &map)
}

/// Whether the document's **track changes** flag is on in `loro`.
#[must_use]
pub fn document_track_changes(loro: &LoroDoc) -> bool {
    let map = loro.get_map(KEY_SETTINGS);
    read_settings(&map).is_some_and(|s| s.track_changes)
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    fn track_changes_flag_round_trips_and_defaults_off() {
        let doc = LoroDoc::new();
        assert!(!document_track_changes(&doc), "absent settings ⇒ off");

        set_track_changes(&doc, true).unwrap();
        assert!(document_track_changes(&doc));

        // Toggling back off preserves the map but clears the flag.
        set_track_changes(&doc, false).unwrap();
        assert!(!document_track_changes(&doc));
    }

    #[test]
    fn setting_track_changes_preserves_other_settings() {
        let doc = LoroDoc::new();
        let map = doc.get_map(KEY_SETTINGS);
        write_settings(
            Some(&DocumentSettings {
                default_tab_stop_pt: 18.0,
                track_changes: false,
            }),
            &map,
        )
        .unwrap();

        set_track_changes(&doc, true).unwrap();
        let back = read_settings(&doc.get_map(KEY_SETTINGS)).expect("settings present");
        assert!(back.track_changes);
        assert_eq!(back.default_tab_stop_pt, 18.0, "tab stop preserved");
    }
}
