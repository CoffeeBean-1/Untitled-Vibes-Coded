use serde::Serialize;
use serde_json::Value;
use std::fs;
use tauri::{AppHandle, Manager, State};

use crate::state::{force_lock, lock, AppState};
use crate::vault::model::{now_ms, Kind, Settings, VaultData};
use crate::vault::store::{atomic_write, validate_backup};
use crate::vault::{self, Vault, VaultError};

type CmdResult<T> = std::result::Result<T, String>;

fn msg<E: ToString>(e: E) -> String {
    e.to_string()
}

fn with_vault<T>(
    st: &AppState,
    f: impl FnOnce(&mut Vault) -> vault::Result<T>,
) -> CmdResult<T> {
    let mut guard = lock(&st.vault);
    let v = guard.as_mut().ok_or_else(|| VaultError::Locked.to_string())?;
    f(v).map_err(msg)
}

/// Applies a change to the vault data and saves it (encrypted) immediately.
fn mutate<T>(st: &AppState, f: impl FnOnce(&mut VaultData) -> vault::Result<T>) -> CmdResult<T> {
    with_vault(st, |v| {
        let r = f(&mut v.data)?;
        v.save()?;
        Ok(r)
    })
}

#[derive(Serialize)]
pub struct Status {
    exists: bool,
    unlocked: bool,
}

#[derive(Serialize, Clone)]
pub struct PageMeta {
    id: String,
    title: String,
    modified: i64,
}

#[derive(Serialize)]
pub struct TreeSection {
    id: String,
    name: String,
    pages: Vec<PageMeta>,
}

#[derive(Serialize)]
pub struct TreeNotebook {
    id: String,
    name: String,
    sections: Vec<TreeSection>,
}

#[derive(Serialize)]
pub struct PageFull {
    id: String,
    title: String,
    content: Value,
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> Status {
    Status { exists: state.path.exists(), unlocked: state.is_unlocked() }
}

#[tauri::command]
pub async fn create_vault(state: State<'_, AppState>, password: String) -> CmdResult<Settings> {
    let v = Vault::create(&state.path, &password).map_err(msg)?;
    let settings = v.data.settings.clone();
    *lock(&state.vault) = Some(v);
    state.touch();
    Ok(settings)
}

#[tauri::command]
pub async fn unlock(state: State<'_, AppState>, password: String) -> CmdResult<Settings> {
    let v = Vault::open(&state.path, &password).map_err(msg)?;
    let settings = v.data.settings.clone();
    *lock(&state.vault) = Some(v);
    state.touch();
    Ok(settings)
}

#[tauri::command]
pub fn lock_vault(app: AppHandle, minimize: Option<bool>) {
    force_lock(&app);
    if minimize.unwrap_or(false) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.minimize();
        }
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    force_lock(&app);
    app.exit(0);
}

#[tauri::command]
pub fn activity(state: State<'_, AppState>) {
    state.touch();
}

#[tauri::command]
pub fn get_tree(state: State<'_, AppState>) -> CmdResult<Vec<TreeNotebook>> {
    with_vault(&state, |v| {
        Ok(v.data
            .notebooks
            .iter()
            .map(|n| TreeNotebook {
                id: n.id.clone(),
                name: n.name.clone(),
                sections: n
                    .sections
                    .iter()
                    .map(|s| TreeSection {
                        id: s.id.clone(),
                        name: s.name.clone(),
                        pages: s
                            .pages
                            .iter()
                            .map(|p| PageMeta { id: p.id.clone(), title: p.title.clone(), modified: p.modified })
                            .collect(),
                    })
                    .collect(),
            })
            .collect())
    })
}

#[tauri::command]
pub fn get_page(state: State<'_, AppState>, id: String) -> CmdResult<PageFull> {
    with_vault(&state, |v| {
        let p = v.data.page_mut(&id)?;
        Ok(PageFull { id: p.id.clone(), title: p.title.clone(), content: p.content.clone() })
    })
}

#[tauri::command]
pub fn save_page(
    state: State<'_, AppState>,
    id: String,
    title: String,
    content: Value,
) -> CmdResult<PageMeta> {
    mutate(&state, |d| {
        let p = d.page_mut(&id)?;
        p.title = title;
        p.content = content;
        p.modified = now_ms();
        Ok(PageMeta { id: p.id.clone(), title: p.title.clone(), modified: p.modified })
    })
}

#[tauri::command]
pub fn create_node(
    state: State<'_, AppState>,
    kind: Kind,
    parent_id: String,
    name: String,
) -> CmdResult<String> {
    mutate(&state, |d| match kind {
        Kind::Notebook => Ok(d.add_notebook(&name)),
        Kind::Section => d.add_section(&parent_id, &name),
        Kind::Page => d.add_page(&parent_id, &name),
    })
}

#[tauri::command]
pub fn rename_node(state: State<'_, AppState>, kind: Kind, id: String, name: String) -> CmdResult<()> {
    mutate(&state, |d| d.rename(kind, &id, &name))
}

#[tauri::command]
pub fn delete_node(state: State<'_, AppState>, kind: Kind, id: String) -> CmdResult<()> {
    mutate(&state, |d| d.delete(kind, &id))
}

#[tauri::command]
pub fn reorder_nodes(
    state: State<'_, AppState>,
    kind: Kind,
    parent_id: String,
    ids: Vec<String>,
) -> CmdResult<()> {
    mutate(&state, |d| d.reorder(kind, &parent_id, &ids))
}

#[tauri::command]
pub fn move_page(state: State<'_, AppState>, page_id: String, section_id: String) -> CmdResult<()> {
    mutate(&state, |d| d.move_page(&page_id, &section_id))
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    with_vault(&state, |v| Ok(v.data.settings.clone()))
}

#[tauri::command]
pub fn set_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    if matches!(settings.idle_minutes, Some(m) if !(1..=60).contains(&m)) {
        return Err("Auto-lock must be between 1 and 60 minutes, or off.".into());
    }
    if !["system", "light", "dark"].contains(&settings.theme.as_str()) {
        return Err("Unknown theme.".into());
    }
    mutate(&state, |d| {
        d.settings = settings;
        Ok(())
    })
}

#[tauri::command]
pub async fn change_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> CmdResult<()> {
    with_vault(&state, |v| v.change_password(&old_password, &new_password))
}

#[tauri::command]
pub fn export_backup(state: State<'_, AppState>, dest: String) -> CmdResult<()> {
    if !state.path.exists() {
        return Err("There is no vault to back up.".into());
    }
    if std::path::Path::new(&dest) == state.path.as_path() {
        return Err("Choose a location other than the live vault file.".into());
    }
    // The vault file is always fully encrypted, so a plain copy is a safe backup.
    fs::copy(&state.path, &dest).map(|_| ()).map_err(msg)
}

/// Replaces the vault with a backup (the current file is kept as vault.dat.bak) and locks.
#[tauri::command]
pub fn import_backup(app: AppHandle, state: State<'_, AppState>, src: String) -> CmdResult<()> {
    let src = std::path::PathBuf::from(src);
    validate_backup(&src).map_err(msg)?;
    let bytes = fs::read(&src).map_err(msg)?;
    force_lock(&app);
    if let Some(dir) = state.path.parent() {
        fs::create_dir_all(dir).map_err(msg)?;
    }
    atomic_write(&state.path, &bytes).map_err(msg)
}
