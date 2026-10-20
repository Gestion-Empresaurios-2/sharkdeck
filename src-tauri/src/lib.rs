//! SharkDeck Tauri app entry.

pub mod commands;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Restore persisted panel/protocol preferences (screen size, pixel order…).
            let cfg = commands::load_config(commands::config_path(app.handle()));
            app.state::<AppState>().config.lock().unwrap().clone_from(&cfg);
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::probe,
            commands::disconnect,
            commands::light_get,
            commands::light_set,
            commands::leds_on,
            commands::options_get,
            commands::options_set,
            commands::profile_get,
            commands::profile_set,
            commands::keymap_read,
            commands::keymap_write,
            commands::macro_read,
            commands::macro_write,
            commands::display_params,
            commands::config_get,
            commands::config_set,
            commands::logs_get,
            commands::layout_get,
            commands::screen_push,
            commands::screen_import,
            commands::screen_pattern,
            commands::screen_clock,
            commands::screen_anim_push,
            commands::pick_open_image,
            commands::pick_open_json,
            commands::pick_save_file,
            commands::keymap_backup,
            commands::keymap_restore,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
