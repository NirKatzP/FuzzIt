use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::fuzzer::engine::{run_fuzz, FuzzHandle, FuzzProgress, FuzzResult};
use crate::fuzzer::wordlist::{self, BUNDLED_WORDLISTS};
use crate::fuzzer::{FilterConfig, FuzzConfig};

/// Holds the handle of whichever run is currently active (or `None`), so
/// `stop_fuzz` has something to cancel. A `Mutex<Option<..>>` managed by
/// Tauri's state system rather than a global `static` — keeps this safe
/// without `unsafe`, and naturally supports "no run active" as a state.
pub struct ActiveRun(pub Mutex<Option<FuzzHandle>>);

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum WordlistSource {
    Bundled(String),
    File(String),
    Inline(Vec<String>),
}

#[derive(Deserialize)]
pub struct StartFuzzArgs {
    pub config: FuzzConfig,
    pub filters: FilterConfig,
    pub wordlist_source: WordlistSource,
}

#[tauri::command]
pub fn list_bundled_wordlists() -> Vec<(String, String)> {
    BUNDLED_WORDLISTS
        .iter()
        .map(|w| (w.id.to_string(), w.label.to_string()))
        .collect()
}

#[tauri::command]
pub async fn start_fuzz(
    app: AppHandle,
    active: State<'_, ActiveRun>,
    args: StartFuzzArgs,
) -> Result<(), String> {
    let words = match args.wordlist_source {
        WordlistSource::Bundled(id) => {
            wordlist::get_bundled(&id).ok_or_else(|| format!("Unknown bundled wordlist: {id}"))?
        }
        WordlistSource::File(path) => wordlist::load_from_file(&path)?,
        WordlistSource::Inline(words) => words,
    };

    if words.is_empty() {
        return Err("The wordlist is empty.".into());
    }

    let handle = FuzzHandle::new();
    {
        let mut guard = active.0.lock().await;
        // A new run supersedes any old one rather than orphaning it.
        if let Some(previous) = guard.replace(handle.clone()) {
            previous.cancel();
        }
    }

    let app_for_results = app.clone();
    let app_for_progress = app.clone();

    let run_outcome = run_fuzz(
        args.config,
        args.filters,
        words,
        handle.clone(),
        move |results: Vec<FuzzResult>| {
            // Errors here just mean the frontend window went away; the
            // fuzz run itself shouldn't fail because of it.
            let _ = app_for_results.emit("fuzz://results", results);
        },
        move |progress: FuzzProgress| {
            let _ = app_for_progress.emit("fuzz://progress", progress);
        },
    )
    .await;

    {
        // Only clear the slot if it's still ours — a newer run may have
        // replaced it, and wiping that would leave Stop with nothing to cancel.
        let mut guard = active.0.lock().await;
        if guard.as_ref().is_some_and(|h| h.same_run(&handle)) {
            *guard = None;
        }
    }
    let _ = app.emit("fuzz://complete", ());

    run_outcome
}

#[tauri::command]
pub async fn stop_fuzz(active: State<'_, ActiveRun>) -> Result<(), String> {
    let guard = active.0.lock().await;
    if let Some(handle) = guard.as_ref() {
        handle.cancel();
    }
    Ok(())
}
