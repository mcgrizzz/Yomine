//! One-time import of the JSON files history lived in before the database.

use std::path::Path;

use rusqlite::Connection;
use serde_json::Value;

use super::{
    batches::{
        self,
        StoredBatch,
    },
    notes::{
        self,
        NewNote,
    },
    now_ms,
    sources::{
        self,
        Open,
    },
};
use crate::anki::mined::normalize_sentence;

/// Reads whatever JSON history exists; a missing or unreadable file imports nothing.
pub(super) fn import_json(conn: &Connection, dir: &Path) -> rusqlite::Result<()> {
    let read = |name: &str| -> Option<Value> {
        let text = std::fs::read_to_string(dir.join(name)).ok()?;
        serde_json::from_str(&text)
            .inspect_err(|e| eprintln!("Skipping {name} in the history import: {e}"))
            .ok()
    };
    let now = now_ms();

    if let Some(batch) = read("yomine_last_batch.json") {
        import_batch(conn, &batch, now)?;
    }
    if let Some(Value::Array(recorded)) = read("yomine_mined_notes.json") {
        for note in recorded {
            if let (Some(id), Some(sentence)) =
                (note["note_id"].as_u64(), note["sentence"].as_str())
            {
                notes::insert(
                    conn,
                    &NewNote {
                        note_id: id,
                        sentence,
                        term: None,
                        source: None,
                        batch_id: None,
                        deleted_at: None,
                    },
                )?;
            }
        }
    }
    // Processing times weren't recorded; the import time stands in.
    if let Some(Value::Array(fingerprints)) = read("processed_media.json") {
        for fingerprint in fingerprints.iter().filter_map(Value::as_str) {
            sources::set_auto_processed(conn, fingerprint, Some(now))?;
        }
    }
    // recent_files.json never recorded fingerprints, so these loads have no source.
    if let Some(Value::Array(files)) = read("recent_files.json").map(|v| v["files"].clone()) {
        for file in &files {
            let opened_at = file["last_opened"]
                .as_str()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map_or(now, |t| t.timestamp_millis());
            let open = Open {
                path: file["file_path"].as_str().unwrap_or_default(),
                title: file["title"].as_str().unwrap_or_default(),
                label: file["subtitle"].as_str(),
                creator: file["creator"].as_str(),
                term_count: file["term_count"].as_i64(),
                file_size: file["file_size"].as_i64(),
                opened_at,
            };
            sources::insert_open(conn, None, &open)?;
        }
    }
    if let Some(Value::Object(books)) = read("epub_history.json") {
        for (path, parts) in &books {
            let parts = parts.as_array().into_iter().flatten().filter_map(Value::as_i64);
            sources::mark_parts_seen(conn, path, parts)?;
        }
    }
    Ok(())
}

fn import_batch(conn: &Connection, batch: &Value, now: i64) -> rusqlite::Result<()> {
    let (Some(id), Some(fingerprint)) =
        (batch["id"].as_str(), batch["source"]["fingerprint"].as_str())
    else {
        return Ok(());
    };
    let items = batch["items"].as_array().cloned().unwrap_or_default();
    let stored = StoredBatch {
        id: id.to_string(),
        fingerprint: fingerprint.to_string(),
        title: batch["source"]["title"].as_str().unwrap_or_default().to_string(),
        path: batch["source"]["locator"].as_str().unwrap_or_default().to_string(),
        started_at: batch["started_at"].as_i64().unwrap_or(now),
        finished_at: batch["finished_at"].as_i64(),
        auto: batch["auto"].as_bool().unwrap_or(false),
        items: items.iter().map(Value::to_string).collect(),
    };
    let source = batches::put(conn, &stored)?;
    sources::set_title(conn, source, &stored.title)?;
    for item in &items {
        let outcome = &item["outcome"];
        let deleted_at = match outcome["status"].as_str() {
            Some("created") => None,
            Some("deleted") => Some(now),
            _ => continue,
        };
        let Some(note_id) = outcome["note_id"].as_u64() else { continue };
        notes::insert(
            conn,
            &NewNote {
                note_id,
                sentence: &normalize_sentence(item["sentence"].as_str().unwrap_or_default()),
                term: item["lemma"].as_str(),
                source: Some(source),
                batch_id: Some(id),
                deleted_at,
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        super::{
            open,
            tests::count,
            DB_FILE,
        },
        *,
    };

    #[test]
    fn first_open_imports_json_history_once_and_keeps_the_files() {
        let dir = std::env::temp_dir().join(format!(
            "yomine-db-import-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let files = [
            (
                "yomine_last_batch.json",
                r#"{"id":"7","started_at":5,"finished_at":9,"auto":true,
                    "source":{"title":"Ep 1","locator":"ep1.srt","fingerprint":"ab"},
                    "items":[
                      {"key":"猫 ねこ","lemma":"猫","sentence":"猫 だ","outcome":{"status":"created","note_id":1}},
                      {"key":"犬 いぬ","lemma":"犬","sentence":"犬","outcome":{"status":"deleted","note_id":2}},
                      {"key":"鳥 とり","lemma":"鳥","sentence":"鳥","outcome":{"status":"duplicate"}}]}"#,
            ),
            (
                "yomine_mined_notes.json",
                r#"[{"note_id":1,"sentence":"猫だ"},{"note_id":3,"sentence":"魚だ"}]"#,
            ),
            ("processed_media.json", r#"["ab","cd"]"#),
            (
                "recent_files.json",
                r#"{"files":[{"file_path":"ep1.srt","title":"Ep 1","subtitle":null,"creator":null,
                    "last_opened":"2026-09-01T10:00:00Z","file_size":10,"term_count":4}]}"#,
            ),
            ("epub_history.json", r#"{"book.epub":[1,3]}"#),
        ];
        for (name, json) in files {
            std::fs::write(dir.join(name), json).unwrap();
        }

        for _ in 0..2 {
            let conn = open(&dir.join(DB_FILE), &dir).unwrap();
            assert_eq!(count(&conn, "SELECT count(*) FROM batch_items WHERE batch_id = '7'"), 3);
            assert_eq!(count(&conn, "SELECT count(*) FROM notes"), 3);
            assert_eq!(count(&conn, "SELECT count(*) FROM notes WHERE batch_id = '7'"), 2);
            assert_eq!(count(&conn, "SELECT count(*) FROM notes WHERE deleted_at IS NOT NULL"), 1);
            assert_eq!(count(&conn, "SELECT count(*) FROM sources WHERE auto_processed_at > 0"), 2);
            assert_eq!(count(&conn, "SELECT opened_at FROM opens"), 1_788_256_800_000);
            assert_eq!(count(&conn, "SELECT count(*) FROM epub_parts_seen"), 2);
        }
        assert!(dir.join("yomine_last_batch.json").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
