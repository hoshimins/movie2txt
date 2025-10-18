use std::path::{Path, PathBuf};
use std::process::Command;
use std::env;
use std::fs;
use tauri::{AppHandle, Emitter};
use serde::{Deserialize, Serialize};

fn get_env_var(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("環境変数 {} が設定されていません", key))
}

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    file_path: String,
    max_line_width: Option<u32>,
) -> Result<String, String> {
    // 環境変数から設定を読み取る
    let ffmpeg_path = get_env_var("FFMPEG_PATH")?;
    let whisper_path = get_env_var("WHISPER_PATH")?;
    let tmp_dir = get_env_var("TMP_DIR")?;
    let out_dir = get_env_var("OUTPUT_DIR")?;

    // パスをPathBufに変換
    let tmp_dir = PathBuf::from(&tmp_dir);
    let out_dir = PathBuf::from(&out_dir);

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
    let wav_path = tmp_dir.join(format!("{}.wav", file_stem));
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

    // 絶対パスを取得（存在する場合）
    let out_dir_absolute = if out_dir.is_absolute() {
        out_dir.clone()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("カレントディレクトリの取得に失敗しました: {}", e))?
            .join(&out_dir)
    };

    let out_dir_str = out_dir_absolute
        .to_str()
        .ok_or("出力ディレクトリパスの変換に失敗しました")?;

    emit_log(&app, &format!("出力ディレクトリ: {}", out_dir_str))?;

    let whisper_args = vec![
        wav_path_str,
        "--model", "large-v3",
        "--language", "ja",
        "--device", "cuda",
        "--compute_type", "float16",
        "--vad_filter", "True",
        "--output_format", "srt",
        "--output_dir", out_dir_str,
    ];

    if let Some(width) = max_line_width {
        if width > 0 {
            emit_log(&app, &format!("1行あたりの最大文字数: {}", width))?;
        }
    }

    let whisper_output = Command::new(&whisper_path)
        .args(&whisper_args)
        .output()
        .map_err(|e| format!("Whisperの実行に失敗しました: {}", e))?;

    emit_log(&app, "Whisper処理が完了しました")?;

    // 出力されたSRTファイルのパス
    let srt_path = out_dir_absolute.join(format!("{}.srt", file_stem));
    let srt_path_str = srt_path
        .to_str()
        .ok_or("SRTファイルパスの変換に失敗しました")?
        .to_string();

    // SRTファイルの存在を確認（終了コードではなくファイルの存在で判断）
    if !srt_path.exists() {
        let stderr = String::from_utf8_lossy(&whisper_output.stderr);
        let stdout = String::from_utf8_lossy(&whisper_output.stdout);
        return Err(format!("SRTファイルが生成されませんでした:\nSTDERR: {}\nSTDOUT: {}", stderr, stdout));
    }

    emit_log(&app, &format!("SRTファイルを生成しました: {}", srt_path_str))?;

    // max_line_widthが指定されている場合、日本語対応の文字数制限を適用
    if let Some(width) = max_line_width {
        if width > 0 {
            emit_log(&app, "文字数制限を適用しています...")?;
            apply_character_limit(&srt_path_str, width as usize)?;
            emit_log(&app, "文字数制限の適用が完了しました")?;
        }
    }

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

/// テキストを指定文字数で分割する
fn split_japanese_text(text: &str, max_chars: usize) -> Vec<String> {
    // 既存の改行を取り除いて1行にする
    let text = text.replace('\n', "").replace('\r', "");

    let mut lines = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let end = (i + max_chars).min(chars.len());
        let line: String = chars[i..end].iter().collect();
        lines.push(line);
        i = end;
    }

    if lines.is_empty() {
        vec![text.to_string()]
    } else {
        lines
    }
}

/// SRTファイルに文字数制限を適用する
fn apply_character_limit(file_path: &str, max_chars: usize) -> Result<(), String> {
    // SRTファイルを読み込む
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;

    let entries = parse_srt(&content)?;
    let mut new_entries = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let lines = split_japanese_text(&entry.text, max_chars);

        // 複数行に分割する場合でも、タイムスタンプは維持して改行で区切る
        let text = lines.join("\n");

        new_entries.push(SubtitleEntry {
            index: index + 1,
            start_time: entry.start_time.clone(),
            end_time: entry.end_time.clone(),
            text,
        });
    }

    // 新しいSRTファイルを書き込む
    let mut content = String::new();
    for entry in new_entries {
        content.push_str(&entry.index.to_string());
        content.push('\n');
        content.push_str(&entry.start_time);
        content.push_str(" --> ");
        content.push_str(&entry.end_time);
        content.push('\n');
        content.push_str(&entry.text);
        content.push_str("\n\n");
    }

    fs::write(file_path, content)
        .map_err(|e| format!("SRTファイルの保存に失敗しました: {}", e))?;

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleEntry {
    pub index: usize,
    pub start_time: String,
    pub end_time: String,
    pub text: String,
}

#[tauri::command]
pub async fn read_srt_file(file_path: String) -> Result<Vec<SubtitleEntry>, String> {
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;

    parse_srt(&content)
}

fn parse_srt(content: &str) -> Result<Vec<SubtitleEntry>, String> {
    let mut entries = Vec::new();

    // Windows/Unix両方の改行コードに対応
    let content = content.replace("\r\n", "\n");
    let blocks: Vec<&str> = content.split("\n\n").filter(|s| !s.trim().is_empty()).collect();

    for block in blocks.iter() {
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 3 {
            continue;
        }

        let index = lines[0].trim().parse::<usize>()
            .map_err(|_| format!("インデックスのパースに失敗しました: {}", lines[0]))?;

        let time_parts: Vec<&str> = lines[1].split(" --> ").collect();
        if time_parts.len() != 2 {
            continue;
        }

        let start_time = time_parts[0].trim().to_string();
        let end_time = time_parts[1].trim().to_string();
        let text = lines[2..].join("\n");

        entries.push(SubtitleEntry {
            index,
            start_time,
            end_time,
            text,
        });
    }

    Ok(entries)
}

#[tauri::command]
pub async fn save_srt_file(file_path: String, entries: Vec<SubtitleEntry>) -> Result<(), String> {
    let mut content = String::new();

    for entry in entries {
        content.push_str(&format!("{}\n", entry.index));
        content.push_str(&format!("{} --> {}\n", entry.start_time, entry.end_time));
        content.push_str(&format!("{}\n\n", entry.text));
    }

    fs::write(&file_path, content)
        .map_err(|e| format!("SRTファイルの保存に失敗しました: {}", e))?;

    Ok(())
}
