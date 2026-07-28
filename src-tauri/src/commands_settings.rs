use crate::{
    application::{
        epoch_secs_now, merge_preferences, RemotePreferencesRepository, SessionRepository,
        StoredSession,
    },
    domain::{AppSettings, SettingsRepository, VerbalixError},
    AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub(crate) fn load_settings(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<AppSettings, VerbalixError> {
    let local = runtime.settings.load()?;

    let Some(repo) = runtime.remote_preferences.as_ref().map(Arc::clone) else {
        return Ok(local);
    };
    let session = match runtime.session.load()? {
        Some(s) => s,
        None => return Ok(local),
    };

    let runtime_clone = Arc::clone(&*runtime);
    tauri::async_runtime::spawn(background_sync(
        app,
        runtime_clone,
        repo,
        session,
        local.clone(),
    ));

    Ok(local)
}

pub(crate) async fn background_sync(
    app: AppHandle,
    runtime: Arc<AppRuntime>,
    repo: Arc<RemotePreferencesRepository>,
    session: StoredSession,
    local: AppSettings,
) {
    let sync_store = &runtime.preferences_sync;

    let captured_seq = sync_store.load().map(|m| m.sequence).unwrap_or(0);

    let remote = match repo.fetch(&session.access_token).await {
        Ok(r) => r,
        Err(_) => return,
    };

    let current_meta = sync_store.load();
    let current_seq = current_meta.as_ref().map(|m| m.sequence).unwrap_or(0);
    if current_seq != captured_seq {
        return;
    }

    let outcome = merge_preferences(&local, current_meta.as_ref(), remote);

    let now_secs = epoch_secs_now();
    let _ = sync_store.record_synced(now_secs);

    if outcome.settings != local {
        let _ = runtime.settings.save(&outcome.settings);
    }

    if let Ok(mut guard) = runtime.synced_settings.lock() {
        *guard = Some(outcome.settings.clone());
    }
    let _ = app.emit("preferences-synced", &outcome.settings);

    if outcome.needs_push {
        let _ = repo.upsert(&outcome.settings, &session.access_token).await;
    }
}

#[tauri::command]
pub(crate) fn current_synced_preferences(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<Option<AppSettings>, VerbalixError> {
    runtime
        .synced_settings
        .lock()
        .map(|g| g.clone())
        .map_err(|_| VerbalixError::LocalFailure)
}

#[tauri::command]
pub(crate) fn save_settings(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
    settings: AppSettings,
) -> Result<(), VerbalixError> {
    use crate::commands::normalized_shortcut;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let now_secs = epoch_secs_now();
    let _ = runtime.preferences_sync.record_change(now_secs);

    runtime.settings.save(&settings)?;
    let shortcut = normalized_shortcut(&settings.shortcut);
    app.global_shortcut()
        .unregister_all()
        .map_err(|_| VerbalixError::LocalFailure)?;
    app.global_shortcut()
        .register(shortcut.as_str())
        .map_err(|_| VerbalixError::LocalFailure)?;
    if let (Some(repo), Ok(Some(session))) = (&runtime.remote_preferences, runtime.session.load()) {
        let repo = Arc::clone(repo);
        let settings = settings.clone();
        tauri::async_runtime::spawn(async move {
            let _ = repo.upsert(&settings, &session.access_token).await;
        });
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn save_session(
    runtime: State<'_, Arc<AppRuntime>>,
    access_token: String,
    refresh_token: String,
) -> Result<(), VerbalixError> {
    runtime.session.save(&StoredSession {
        access_token,
        refresh_token,
    })
}

#[tauri::command]
pub(crate) fn has_session(runtime: State<'_, Arc<AppRuntime>>) -> Result<bool, VerbalixError> {
    runtime.session.load().map(|session| session.is_some())
}

#[tauri::command]
pub(crate) fn clear_session(runtime: State<'_, Arc<AppRuntime>>) -> Result<(), VerbalixError> {
    runtime.session.clear()
}
