pub mod commands;
pub mod state;
pub mod vault;
#[cfg(windows)]
mod winevents;

use std::path::PathBuf;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

use state::{force_lock, lock, request_lock, AppState};

fn webview_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("SecureNotes").join("webview"))
}

fn wipe_webview_dir() {
    if let Some(d) = webview_dir() {
        let _ = std::fs::remove_dir_all(d);
    }
}

pub fn run() {
    // Keep the webview's profile (cache, cookies, etc.) in a folder we delete on start and exit.
    // Notes themselves never touch web storage.
    wipe_webview_dir();
    if let Some(d) = webview_dir() {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", d);
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .setup(|app| {
            state::start_idle_watcher(app.handle().clone());
            #[cfg(windows)]
            winevents::start(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match event {
                WindowEvent::Resized(_) if window.is_minimized().unwrap_or(false) => {
                    let st = app.state::<AppState>();
                    let lock_on_min =
                        lock(&st.vault).as_ref().map(|v| v.data.settings.lock_on_minimize).unwrap_or(false);
                    if lock_on_min {
                        request_lock(app);
                    }
                }
                WindowEvent::CloseRequested { api, .. } => {
                    if app.state::<AppState>().is_unlocked() {
                        // Let the UI flush unsaved edits, then quit (forced after a grace period).
                        api.prevent_close();
                        let _ = app.emit("flush-and-quit", ());
                        let app = app.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            force_lock(&app);
                            app.exit(0);
                        });
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::create_vault,
            commands::unlock,
            commands::lock_vault,
            commands::quit_app,
            commands::activity,
            commands::get_tree,
            commands::get_page,
            commands::save_page,
            commands::create_node,
            commands::rename_node,
            commands::delete_node,
            commands::reorder_nodes,
            commands::move_page,
            commands::get_settings,
            commands::set_settings,
            commands::change_password,
            commands::export_backup,
            commands::import_backup,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Note-inator");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            let st = handle.state::<AppState>();
            lock(&st.vault).take(); // drops the vault; the key is zeroized
            wipe_webview_dir();
        }
    });
}
