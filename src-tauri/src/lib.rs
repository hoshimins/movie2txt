mod application;
mod commands;
mod domain;
mod infrastructure;
mod storage;

#[cfg(debug_assertions)]
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // .envファイルを読み込む
    dotenvy::dotenv().ok();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build());

    #[cfg(target_os = "windows")]
    let builder = builder.plugin(tauri_plugin_log::Builder::new().build());

    builder
        .manage(commands::JobRegistry::default())
        .invoke_handler(tauri::generate_handler![
            commands::create_project,
            commands::list_projects,
            commands::open_project,
            commands::attach_existing_media,
            commands::start_preparation_job,
            commands::start_media_download_job,
            commands::preview_audio_split,
            commands::start_audio_split_job,
            commands::scan_audio_merge,
            commands::start_audio_merge_job,
            commands::start_vocals_transcription_job,
            commands::cancel_job,
            commands::read_subtitles,
            commands::save_subtitles,
            commands::save_clip_markers,
            commands::detect_silence,
            commands::create_silence_cut,
            commands::generate_highlight_request,
            commands::import_highlight_candidates,
            commands::get_app_settings,
            commands::update_app_settings,
            commands::update_ytdlp,
            commands::start_transcription,
            commands::open_srt_file,
            commands::open_path,
            commands::read_srt_file,
            commands::save_srt_file,
        ])
        .setup(|_app| {
            #[cfg(debug_assertions)]
            {
                let window = _app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
