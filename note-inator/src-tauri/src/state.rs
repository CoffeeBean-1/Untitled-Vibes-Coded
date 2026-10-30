use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::vault::Vault;

/// How long the UI gets to flush unsaved edits before we lock regardless.
const FLUSH_GRACE: Duration = Duration::from_secs(2);

pub struct AppState {
    pub vault: Mutex<Option<Vault>>,
    pub last_activity: Mutex<Instant>,
    pub path: PathBuf,
}

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub fn new() -> Self {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        AppState {
            vault: Mutex::new(None),
            last_activity: Mutex::new(Instant::now()),
            path: base.join("SecureNotes").join("vault.dat"),
        }
    }

    pub fn is_unlocked(&self) -> bool {
        lock(&self.vault).is_some()
    }

    pub fn touch(&self) {
        *lock(&self.last_activity) = Instant::now();
    }
}

/// Drops the vault (zeroizing the key) and tells the UI to wipe its decrypted state.
pub fn force_lock(app: &AppHandle) {
    let state = app.state::<AppState>();
    let was_unlocked = lock(&state.vault).take().is_some();
    if was_unlocked {
        let _ = app.emit("locked", ());
    }
}

/// Asks the UI to flush pending edits and lock; locks anyway after a short grace period.
pub fn request_lock(app: &AppHandle) {
    if !app.state::<AppState>().is_unlocked() {
        return;
    }
    let _ = app.emit("lock-now", ());
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(FLUSH_GRACE);
        force_lock(&app);
    });
}

/// Idle auto-lock. The timer lives here, not in the webview, so throttled JS can't delay it.
pub fn start_idle_watcher(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));
        let state = app.state::<AppState>();
        let minutes = match lock(&state.vault).as_ref() {
            Some(v) => v.data.settings.idle_minutes,
            None => continue,
        };
        if let Some(m) = minutes {
            if lock(&state.last_activity).elapsed() >= Duration::from_secs(m as u64 * 60) {
                state.touch();
                request_lock(&app);
            }
        }
    });
}
