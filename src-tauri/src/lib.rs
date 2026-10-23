pub mod graphics;
pub mod display;
pub mod commands;
pub mod audio;
pub mod midi;
pub mod compute;
pub mod sharing;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // Initialize WGPU engine state
      match display::EngineState::new_headless() {
        Ok(engine_state) => {
          app.manage(engine_state);
        }
        Err(e) => {
          log::warn!("WGPU context initialization deferred or unavailable: {}", e);
        }
      }

      // Hook projector window close request to gracefully hide it
      if let Some(proj_win) = app.get_webview_window("projector") {
        let proj_clone = proj_win.clone();
        let app_handle = app.handle().clone();
        proj_win.on_window_event(move |event| {
          if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = proj_clone.hide();
            let _ = app_handle.emit("projector:closed", ());
          }
        });
      }

      // Explicitly reveal and focus the control window on desktop launch
      if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.show();
        let _ = main_win.set_focus();
      }

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::get_monitors,
      commands::open_projector,
      commands::close_projector,
      commands::toggle_projector_fullscreen,
      commands::get_engine_status,
      commands::save_project_file,
      commands::read_project_file,
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
