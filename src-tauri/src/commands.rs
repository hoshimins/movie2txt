use std::path::{Path, PathBuf};
use std::process::Command;
use std::env;
use tauri::{AppHandle, Emitter};

fn get_env_var(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("環境変数 {} が設定されていません", key))
}

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    file_path: String,
) -> Result<String, String> {
    // 環境変数から設定を読み取る
    let ffmpeg_path = get_env_var("FFMPEG_PATH")?;
    let whisper_path = get_env_var("WHISPER_PATH")?;
    let tmp_dir = get_env_var("TMP_DIR")?;
    let out_dir = get_env_var("OUT_DIR")?;

    // ディレクトリの存在確認と作成
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("tmpディレクトリの作成に失敗しました: {}", e))?;
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("outディレクトリの作成に失敗しました: {}", e))?;

    let input_path = Path::new(&file_path);
    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("ファイル名の取得に失敗しました")?;

    // 中間wavファイルのパス
    let wav_path = PathBuf::from(&tmp_dir).join(format!("{}.wav", file_stem));
    let wav_path_str = wav_path
        .to_str()
        .ok_or("wavファイルパスの変換に失敗しました")?;

    // Step 1: ffmpeg で 16kHz mono に変換
    emit_log(&app, "ffmpeg処理を開始します...")?;

    let ffmpeg_output = Command::new(&ffmpeg_path)
        .args(&[
            "-i", &file_path,
            "-ar", "16000",
            "-ac", "1",
            "-y",
            wav_path_str,
        ])
        .output()
        .map_err(|e| format!("ffmpegの実行に失敗しました: {}", e))?;

    if !ffmpeg_output.status.success() {
        let stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
        return Err(format!("ffmpeg処理に失敗しました: {}", stderr));
    }

    emit_log(&app, "ffmpeg処理が完了しました")?;

    // Step 2: faster-whisper で文字起こし
    emit_log(&app, "Whisper処理を開始します...")?;

    let whisper_output = Command::new(&whisper_path)
        .args(&[
            wav_path_str,
            "--model", "large-v3",
            "--language", "ja",
            "--device", "cuda",
            "--compute_type", "float16",
            "--vad",
            "--output_format", "srt",
            "--output_dir", &out_dir,
        ])
        .output()
        .map_err(|e| format!("Whisperの実行に失敗しました: {}", e))?;

    if !whisper_output.status.success() {
        let stderr = String::from_utf8_lossy(&whisper_output.stderr);
        return Err(format!("Whisper処理に失敗しました: {}", stderr));
    }

    emit_log(&app, "Whisper処理が完了しました")?;

    // 出力されたSRTファイルのパス
    let srt_path = PathBuf::from(&out_dir).join(format!("{}.srt", file_stem));
    let srt_path_str = srt_path
        .to_str()
        .ok_or("SRTファイルパスの変換に失敗しました")?
        .to_string();

    if !srt_path.exists() {
        return Err("SRTファイルが生成されませんでした".to_string());
    }

    emit_log(&app, &format!("SRTファイルを生成しました: {}", srt_path_str))?;

    Ok(srt_path_str)
}

#[tauri::command]
pub async fn open_srt_file(file_path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("ファイルを開けませんでした: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        return Err("この機能はWindowsでのみ利用可能です".to_string());
    }

    Ok(())
}

fn emit_log(app: &AppHandle, message: &str) -> Result<(), String> {
    app.emit("transcription-log", message)
        .map_err(|e| format!("ログの送信に失敗しました: {}", e))
}
