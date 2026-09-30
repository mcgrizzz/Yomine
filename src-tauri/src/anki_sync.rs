//! Syncs Yomine's copy of the Anki collection, then brings the loaded file's table and the
//! mined state up to date with it.

use std::{
    collections::HashSet,
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
        Mutex,
    },
    time::Duration,
};

use tauri::{
    AppHandle,
    Emitter,
    Manager,
};
use yomine::{
    anki::{
        self,
        comprehensibility::calculate_sentence_comprehension,
        mined,
        AnkiState,
        NoteEvent,
    },
    core::pipeline::{
        apply_filters,
        refilter_known,
        AnkiFilter,
    },
    persistence::db,
};

use crate::{
    batches::BatchSource,
    commands::file::load_result,
    dto::MinedStateDto,
    events::{
        names,
        AnkiStatus,
    },
    state::AppState,
};

pub(crate) enum Refresh {
    /// After Anki changed: rows mined or ignored since the last full refresh stay shown.
    Live,
    /// 🔄: the ignore list and Anki's cards both apply to every row.
    Full,
}

pub(crate) enum RefreshOutcome {
    Done,
    /// Yomine's copy has no cards; only 🔄 reports it, to avoid a banner before setup.
    NoVocab,
}

/// Syncs, and refreshes the table live when the copy changed. `progress` shows the Anki
/// spinner while it runs.
pub(crate) async fn sync(app: &AppHandle, progress: bool) -> Result<(), String> {
    let mapping =
        app.state::<Mutex<AppState>>().lock().unwrap().settings.anki_model_mappings.clone();
    if progress {
        let _ = app.emit(names::ANKI_STATUS, AnkiStatus { connected: true, fetching: true });
    }
    let result = anki::sync::sync(&mapping).await;
    if progress {
        let connected = result.is_ok();
        let _ = app.emit(names::ANKI_STATUS, AnkiStatus { connected, fetching: false });
    }
    if result.map_err(|e| format!("Anki is unreachable: {e}"))? {
        refresh(app, Refresh::Live).await?;
    }
    Ok(())
}

/// Set while Tsunagi's event stream is connected, which makes background syncs unneeded.
static FOLLOWING: AtomicBool = AtomicBool::new(false);

const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// A sync in the background, for a moment Anki may have changed.
pub(crate) fn hint(app: &AppHandle) {
    if FOLLOWING.load(Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync(&app, false).await {
            eprintln!("Anki sync: {e}");
        }
    });
}

/// Follows Tsunagi's event stream while the configured connection is Tsunagi, so changes
/// reach Yomine's copy as they happen. Logs only when following starts or stops, or the
/// reason it can't changes.
pub(crate) fn follow_events(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_failure = None;
        loop {
            let anki = anki::current();
            if anki.is_configured_tsunagi() {
                let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
                let connection_changed = async {
                    while anki.is_configured_tsunagi() {
                        tokio::time::sleep(RECONNECT_DELAY).await;
                    }
                };
                let reason = tokio::select! {
                    result = anki.follow_notes(|event| drop(sender.send(event))) => match result {
                        Ok(Some(reason)) => format!("Tsunagi closed the stream ({reason})"),
                        Ok(None) => "the stream ended".to_string(),
                        Err(e) => e.to_string(),
                    },
                    _ = connection_changed => "the connection or add-on changed".to_string(),
                    _ = apply_events(&app, receiver) => "the stream ended".to_string(),
                };
                if FOLLOWING.swap(false, Ordering::Relaxed) {
                    eprintln!("Stopped following Tsunagi's changes: {reason}");
                } else if last_failure.as_ref() != Some(&reason) {
                    eprintln!("Can't follow Tsunagi's changes: {reason}");
                }
                last_failure = Some(reason);
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    });
}

async fn apply_events(
    app: &AppHandle,
    mut receiver: tokio::sync::mpsc::UnboundedReceiver<NoteEvent>,
) {
    while let Some(first) = receiver.recv().await {
        // Messages that arrived during the last sync are applied together.
        let mut events = vec![first];
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        let (mut catch_up, mut changed, mut deleted) = (false, Vec::new(), Vec::new());
        for event in events {
            match event {
                // Messages sent while Yomine wasn't listening aren't replayed.
                NoteEvent::Ready => {
                    if !FOLLOWING.swap(true, Ordering::Relaxed) {
                        println!("Following Tsunagi's changes");
                    }
                    catch_up = true;
                }
                NoteEvent::Stale => catch_up = true,
                NoteEvent::Changed(ids) => changed.extend(ids),
                NoteEvent::Deleted(ids) => deleted.extend(ids),
            }
        }
        let mapping =
            app.state::<Mutex<AppState>>().lock().unwrap().settings.anki_model_mappings.clone();
        let result = if catch_up {
            anki::sync::sync(&mapping).await
        } else if !changed.is_empty() || !deleted.is_empty() {
            anki::sync::sync_changed(&mapping, &changed, &deleted).await
        } else {
            continue;
        };
        match result {
            Ok(true) => {
                if let Err(e) = refresh(app, Refresh::Live).await {
                    eprintln!("Anki refresh: {e}");
                }
            }
            Ok(false) => {}
            Err(e) => eprintln!("Anki sync: {e}"),
        }
    }
}

/// Rebuilds the known words from Yomine's copy, re-filters the loaded file against them
/// and pushes the mined state.
pub(crate) async fn refresh(app: &AppHandle, mode: Refresh) -> Result<RefreshOutcome, String> {
    let state = app.state::<Mutex<AppState>>();
    let (tools, frequency_manager, known_interval) = {
        let guard = state.lock().unwrap();
        let Some(tools) = guard.language_tools.clone() else {
            // Known words are built from the copy once the tools load.
            return match mode {
                Refresh::Live => Ok(RefreshOutcome::Done),
                Refresh::Full => Err("Language tools are still loading".to_string()),
            };
        };
        let frequency_manager = tools.frequency_manager.clone();
        let known_interval = tools.known_interval;
        (tools, frequency_manager, known_interval)
    };
    let Some(anki_state) = AnkiState::from_cache(frequency_manager, known_interval).map(Arc::new)
    else {
        return Ok(match mode {
            Refresh::Live => RefreshOutcome::Done,
            Refresh::Full => RefreshOutcome::NoVocab,
        });
    };

    let (base_terms, shown, known_lemmas, mut sentences, fingerprint, revisions) = {
        let mut guard = state.lock().unwrap();
        guard.set_anki_state(anki_state.clone());
        guard.knowledge_dirty.store(true, Ordering::Relaxed);
        let pending = guard
            .dictionary_refresh_pending
            .as_ref()
            .is_some_and(|revision| Arc::ptr_eq(revision, &guard.input_revision));
        if guard.file.base_terms.is_empty() || pending {
            drop(guard);
            push_mined_state(app);
            return Ok(RefreshOutcome::Done);
        }
        (
            guard.file.base_terms.clone(),
            guard.file.terms.clone(),
            guard.file.anki_known_lemmas.clone(),
            guard.file.sentences.clone(),
            BatchSource::from_file(&guard.file).map(|s| s.fingerprint).ok(),
            (guard.file.revision.clone(), guard.input_revision.clone()),
        )
    };

    let filtered = match mode {
        Refresh::Full => apply_filters(base_terms, &tools, AnkiFilter::Snapshot(Some(anki_state)))
            .await
            .map_err(|e| e.to_string())?,
        Refresh::Live => {
            let mined: HashSet<String> =
                fingerprint.map(|f| mined_lemmas(&f)).unwrap_or_default().into_iter().collect();
            let ignored = tools.ignore_list.lock().map_err(|_| "Failed to lock ignore list")?;
            let keep = |t: &yomine::core::Term| {
                ignored.contains(&t.lemma_form) || mined.contains(&t.lemma_form)
            };
            refilter_known(base_terms, &shown, &known_lemmas, keep, &anki_state)
        }
    };

    let mut all_terms = Vec::new();
    all_terms.extend(filtered.terms.iter().cloned());
    all_terms.extend(filtered.anki_filtered.iter().cloned());
    all_terms.extend(filtered.ignore_filtered.iter().cloned());
    for sentence in &mut sentences {
        calculate_sentence_comprehension(sentence, &all_terms);
    }
    let file_comprehension = if sentences.is_empty() {
        0.0
    } else {
        sentences.iter().map(|s| s.comprehension).sum::<f32>() / sentences.len() as f32
    };

    {
        let mut guard = state.lock().unwrap();
        if guard.file_update_is_current(&revisions.0, &revisions.1) {
            guard.file.revision = Arc::new(());
            guard.file.anki_known_lemmas =
                filtered.anki_filtered.iter().map(|t| t.lemma_form.clone()).collect();
            guard.file.ignored_count = filtered.ignore_filtered.len();
            guard.file.terms = filtered.terms;
            guard.file.base_terms = all_terms;
            guard.file.sentences = sentences;
            guard.file.file_comprehension = file_comprehension;
            if let Some(payload) = load_result(&guard.file) {
                let _ = app.emit(names::TERMS_REFRESHED, payload);
            }
        }
    }
    push_mined_state(app);
    Ok(RefreshOutcome::Done)
}

/// Lemmas mined from a source: Yomine's record of the notes it created, and those the
/// running batch hasn't recorded yet.
fn mined_lemmas(fingerprint: &str) -> Vec<String> {
    let mut lemmas = mined::mined_terms(fingerprint);
    if let Ok(collection) = db::with(|conn| db::anki::active_collection(conn)) {
        lemmas.extend(crate::batches::unrecorded_lemmas(&collection, fingerprint));
    }
    lemmas.sort_unstable();
    lemmas.dedup();
    lemmas
}

pub(crate) fn mined_state(state: &AppState) -> MinedStateDto {
    let terms = BatchSource::from_file(&state.file)
        .map(|source| mined_lemmas(&source.fingerprint))
        .unwrap_or_default();
    MinedStateDto { mined_terms: terms, mined_sentences: mined::mined_sentences() }
}

fn push_mined_state(app: &AppHandle) {
    let dto = mined_state(&app.state::<Mutex<AppState>>().lock().unwrap());
    let _ = app.emit(names::MINED_STATE, dto);
}
