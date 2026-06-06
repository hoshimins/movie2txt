use crate::application::{
    build_audio_split_preview_for_project, create_silence_cut_for_loaded_project,
    detect_silence_for_project, generate_highlight_request_for_project,
    import_highlight_candidates_from_file, progress_payload, run_audio_merge_job,
    run_audio_split_job, run_media_download_job, run_preparation_job, run_vocals_transcription_job,
    run_ytdlp_update, scan_audio_merge_folder,
};
use crate::domain::{
    AppSettings, AudioMergePreview, AudioMergeTarget, AudioSplitPreview, ClipMarker,
    DownloadSource, HighlightRequestBundle, HighlightRequestOptions, JobId, JobPhase, JobStatus,
    PreparationOptions, Project, ProjectSnapshot, ProjectSummary, SilenceAnalysis,
    SilenceCutResult, SilenceCutSettings, SubtitleDocument,
};
use crate::infrastructure::{SystemProcessRunner, ToolResolver};
use crate::storage::ProjectRepository;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use subtitle_processing::{apply_character_limit_to_content, parse_srt, render_srt, SubtitleEntry};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[derive(Default)]
pub struct JobRegistry {
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl JobRegistry {
    fn insert(&self, job_id: String, cancelled: Arc<AtomicBool>) -> Result<(), String> {
        self.jobs
            .lock()
            .map_err(|_| "ジョブ状態のロックに失敗しました".to_string())?
            .insert(job_id, cancelled);
        Ok(())
    }

    fn cancel(&self, job_id: &str) -> Result<(), String> {
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| "ジョブ状態のロックに失敗しました".to_string())?;
        let Some(cancelled) = jobs.get(job_id) else {
            return Err("指定されたジョブが見つかりません".to_string());
        };
        cancelled.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn remove(&self, job_id: &str) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(job_id);
        }
    }
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    name: String,
    source: DownloadSource,
) -> Result<Project, String> {
    repository_for(&app)?.create_project(name, source)
}

#[tauri::command]
pub async fn list_projects(app: AppHandle) -> Result<Vec<ProjectSummary>, String> {
    repository_for(&app)?.list_projects()
}

#[tauri::command]
pub async fn open_project(app: AppHandle, project_id: String) -> Result<ProjectSnapshot, String> {
    repository_for(&app)?.open_project(&project_id)
}

#[tauri::command]
pub async fn attach_existing_media(
    app: AppHandle,
    project_id: String,
    path: String,
) -> Result<Project, String> {
    let project = repository_for(&app)?.attach_existing_media(&project_id, path)?;
    app.emit("project-updated", project.id.clone())
        .map_err(|e| format!("プロジェクト更新通知に失敗しました: {}", e))?;
    Ok(project)
}

#[tauri::command]
pub async fn start_preparation_job(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    project_id: String,
    options: PreparationOptions,
) -> Result<JobId, String> {
    let repository = repository_for(&app)?;
    let settings = repository.load_settings()?;
    let resource_dir = app.path().resource_dir().ok();
    let job_id = Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    registry.insert(job_id.clone(), cancelled.clone())?;

    let app_for_task = app.clone();
    let job_id_for_task = job_id.clone();
    let project_id_for_task = project_id.clone();
    tauri::async_runtime::spawn(async move {
        let runner = SystemProcessRunner;
        let emit_progress = |phase: JobPhase, status: JobStatus, message: String| {
            let payload = progress_payload(
                &job_id_for_task,
                &project_id_for_task,
                phase,
                status,
                message,
            );
            let _ = app_for_task.emit("job-progress", payload);
        };

        let result = run_preparation_job(
            repository.clone(),
            project_id_for_task.clone(),
            options,
            settings,
            resource_dir,
            &runner,
            cancelled,
            &emit_progress,
        );

        match result {
            Ok(project) => {
                let _ = app_for_task.emit("project-updated", project.id);
            }
            Err(error) => {
                let phase = if error.contains("キャンセル") {
                    JobPhase::Cancelled
                } else {
                    JobPhase::Failed
                };
                let status = if matches!(&phase, JobPhase::Cancelled) {
                    JobStatus::Cancelled
                } else {
                    JobStatus::Failed
                };
                emit_progress(phase, status, error);
            }
        }

        if let Some(registry) = app_for_task.try_state::<JobRegistry>() {
            registry.remove(&job_id_for_task);
        }
    });

    Ok(JobId { id: job_id })
}

#[tauri::command]
pub async fn start_media_download_job(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    project_id: String,
) -> Result<JobId, String> {
    let repository = repository_for(&app)?;
    let settings = repository.load_settings()?;
    let resource_dir = app.path().resource_dir().ok();
    let job_id = Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    registry.insert(job_id.clone(), cancelled.clone())?;

    let app_for_task = app.clone();
    let job_id_for_task = job_id.clone();
    let project_id_for_task = project_id.clone();
    tauri::async_runtime::spawn(async move {
        let runner = SystemProcessRunner;
        let emit_progress = |phase: JobPhase, status: JobStatus, message: String| {
            let payload = progress_payload(
                &job_id_for_task,
                &project_id_for_task,
                phase,
                status,
                message,
            );
            let _ = app_for_task.emit("job-progress", payload);
        };

        let result = run_media_download_job(
            repository.clone(),
            project_id_for_task.clone(),
            settings,
            resource_dir,
            &runner,
            cancelled,
            &emit_progress,
        );
        finish_project_job(&app_for_task, &job_id_for_task, result, &emit_progress);
    });

    Ok(JobId { id: job_id })
}

#[tauri::command]
pub async fn preview_audio_split(
    app: AppHandle,
    project_id: String,
    split_minutes: u32,
) -> Result<AudioSplitPreview, String> {
    let repository = repository_for(&app)?;
    let resolver = ToolResolver::new(app.path().resource_dir().ok(), repository.load_settings()?);
    build_audio_split_preview_for_project(&repository, &project_id, split_minutes, &resolver)
}

#[tauri::command]
pub async fn start_audio_split_job(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    project_id: String,
    split_minutes: u32,
) -> Result<JobId, String> {
    let repository = repository_for(&app)?;
    let settings = repository.load_settings()?;
    let resource_dir = app.path().resource_dir().ok();
    let job_id = Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    registry.insert(job_id.clone(), cancelled.clone())?;

    let app_for_task = app.clone();
    let job_id_for_task = job_id.clone();
    let project_id_for_task = project_id.clone();
    tauri::async_runtime::spawn(async move {
        let runner = SystemProcessRunner;
        let emit_progress = |phase: JobPhase, status: JobStatus, message: String| {
            let payload = progress_payload(
                &job_id_for_task,
                &project_id_for_task,
                phase,
                status,
                message,
            );
            let _ = app_for_task.emit("job-progress", payload);
        };

        let result = run_audio_split_job(
            repository.clone(),
            project_id_for_task.clone(),
            split_minutes,
            settings,
            resource_dir,
            &runner,
            cancelled,
            &emit_progress,
        );
        finish_project_job(&app_for_task, &job_id_for_task, result, &emit_progress);
    });

    Ok(JobId { id: job_id })
}

#[tauri::command]
pub async fn scan_audio_merge(source_dir: String) -> Result<AudioMergePreview, String> {
    scan_audio_merge_folder(&source_dir)
}

#[tauri::command]
pub async fn start_audio_merge_job(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    project_id: String,
    target: AudioMergeTarget,
    source_dir: String,
) -> Result<JobId, String> {
    let repository = repository_for(&app)?;
    let settings = repository.load_settings()?;
    let resource_dir = app.path().resource_dir().ok();
    let job_id = Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    registry.insert(job_id.clone(), cancelled.clone())?;

    let app_for_task = app.clone();
    let job_id_for_task = job_id.clone();
    let project_id_for_task = project_id.clone();
    tauri::async_runtime::spawn(async move {
        let runner = SystemProcessRunner;
        let emit_progress = |phase: JobPhase, status: JobStatus, message: String| {
            let payload = progress_payload(
                &job_id_for_task,
                &project_id_for_task,
                phase,
                status,
                message,
            );
            let _ = app_for_task.emit("job-progress", payload);
        };

        let result = run_audio_merge_job(
            repository.clone(),
            project_id_for_task.clone(),
            target,
            source_dir,
            settings,
            resource_dir,
            &runner,
            cancelled,
            &emit_progress,
        );
        finish_project_job(&app_for_task, &job_id_for_task, result, &emit_progress);
    });

    Ok(JobId { id: job_id })
}

#[tauri::command]
pub async fn start_vocals_transcription_job(
    app: AppHandle,
    registry: State<'_, JobRegistry>,
    project_id: String,
    max_line_width: Option<u32>,
) -> Result<JobId, String> {
    let repository = repository_for(&app)?;
    let settings = repository.load_settings()?;
    let resource_dir = app.path().resource_dir().ok();
    let job_id = Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    registry.insert(job_id.clone(), cancelled.clone())?;

    let app_for_task = app.clone();
    let job_id_for_task = job_id.clone();
    let project_id_for_task = project_id.clone();
    tauri::async_runtime::spawn(async move {
        let runner = SystemProcessRunner;
        let emit_progress = |phase: JobPhase, status: JobStatus, message: String| {
            let payload = progress_payload(
                &job_id_for_task,
                &project_id_for_task,
                phase,
                status,
                message,
            );
            let _ = app_for_task.emit("job-progress", payload);
        };

        let result = run_vocals_transcription_job(
            repository.clone(),
            project_id_for_task.clone(),
            max_line_width,
            settings,
            resource_dir,
            &runner,
            cancelled,
            &emit_progress,
        );
        finish_project_job(&app_for_task, &job_id_for_task, result, &emit_progress);
    });

    Ok(JobId { id: job_id })
}

#[tauri::command]
pub async fn cancel_job(registry: State<'_, JobRegistry>, job_id: String) -> Result<(), String> {
    registry.cancel(&job_id)
}

#[tauri::command]
pub async fn read_subtitles(
    app: AppHandle,
    project_id: String,
) -> Result<SubtitleDocument, String> {
    Ok(repository_for(&app)?.open_project(&project_id)?.subtitles)
}

#[tauri::command]
pub async fn save_subtitles(
    app: AppHandle,
    project_id: String,
    entries: Vec<SubtitleEntry>,
) -> Result<Project, String> {
    let project = repository_for(&app)?.save_subtitles(&project_id, &entries)?;
    app.emit("project-updated", project.id.clone())
        .map_err(|e| format!("プロジェクト更新通知に失敗しました: {}", e))?;
    Ok(project)
}

#[tauri::command]
pub async fn save_clip_markers(
    app: AppHandle,
    project_id: String,
    markers: Vec<ClipMarker>,
) -> Result<Project, String> {
    let project = repository_for(&app)?.save_markers(&project_id, markers)?;
    app.emit("project-updated", project.id.clone())
        .map_err(|e| format!("プロジェクト更新通知に失敗しました: {}", e))?;
    Ok(project)
}

#[tauri::command]
pub async fn detect_silence(
    app: AppHandle,
    project_id: String,
    settings: SilenceCutSettings,
) -> Result<SilenceAnalysis, String> {
    let repository = repository_for(&app)?;
    let resolver = ToolResolver::new(app.path().resource_dir().ok(), repository.load_settings()?);
    let runner = SystemProcessRunner;
    let cancelled = Arc::new(AtomicBool::new(false));
    let emit_progress = |phase: JobPhase, status: JobStatus, message: String| {
        let payload = progress_payload("manual", &project_id, phase, status, message);
        let _ = app.emit("job-progress", payload);
    };

    detect_silence_for_project(
        &repository,
        &project_id,
        settings,
        &resolver,
        &runner,
        cancelled,
        &emit_progress,
    )
}

#[tauri::command]
pub async fn create_silence_cut(
    app: AppHandle,
    project_id: String,
    settings: SilenceCutSettings,
) -> Result<SilenceCutResult, String> {
    let repository = repository_for(&app)?;
    let resolver = ToolResolver::new(app.path().resource_dir().ok(), repository.load_settings()?);
    let runner = SystemProcessRunner;
    let cancelled = Arc::new(AtomicBool::new(false));
    let emit_progress = |phase: JobPhase, status: JobStatus, message: String| {
        let payload = progress_payload("manual", &project_id, phase, status, message);
        let _ = app.emit("job-progress", payload);
    };

    let result = create_silence_cut_for_loaded_project(
        &repository,
        &project_id,
        settings,
        &resolver,
        &runner,
        cancelled,
        &emit_progress,
    )?;
    app.emit("project-updated", project_id)
        .map_err(|e| format!("プロジェクト更新通知に失敗しました: {}", e))?;
    Ok(result)
}

#[tauri::command]
pub async fn generate_highlight_request(
    app: AppHandle,
    project_id: String,
    options: HighlightRequestOptions,
) -> Result<HighlightRequestBundle, String> {
    let repository = repository_for(&app)?;
    let bundle = generate_highlight_request_for_project(&repository, &project_id, options)?;
    app.emit("project-updated", project_id)
        .map_err(|e| format!("プロジェクト更新通知に失敗しました: {}", e))?;
    Ok(bundle)
}

#[tauri::command]
pub async fn import_highlight_candidates(
    _app: AppHandle,
    _project_id: String,
    file_path: String,
) -> Result<Vec<ClipMarker>, String> {
    import_highlight_candidates_from_file(&file_path)
}

#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    repository_for(&app)?.load_settings()
}

#[tauri::command]
pub async fn update_app_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    repository_for(&app)?.save_settings(&settings)
}

#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<(), String> {
    let repository = repository_for(&app)?;
    let settings = repository.load_settings()?;
    let resolver = ToolResolver::new(app.path().resource_dir().ok(), settings);
    let runner = SystemProcessRunner;
    let cancelled = Arc::new(AtomicBool::new(false));

    run_ytdlp_update(&resolver, &runner, cancelled, &mut |line| {
        let _ = emit_log(&app, &line);
    })
}

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    file_path: String,
    max_line_width: Option<u32>,
) -> Result<String, String> {
    let ffmpeg_path = get_env_var("FFMPEG_PATH")?;
    let whisper_path = get_env_var("WHISPER_PATH")?;
    let tmp_dir = get_env_var("TMP_DIR")?;
    let out_dir = get_env_var("OUTPUT_DIR")?;

    let tmp_dir = PathBuf::from(&tmp_dir);
    let out_dir = PathBuf::from(&out_dir);

    fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("tmpディレクトリの作成に失敗しました: {}", e))?;
    fs::create_dir_all(&out_dir)
        .map_err(|e| format!("outディレクトリの作成に失敗しました: {}", e))?;

    let input_path = Path::new(&file_path);
    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("ファイル名の取得に失敗しました")?;

    let wav_path = tmp_dir.join(format!("{}.wav", file_stem));
    let wav_path_str = wav_path
        .to_str()
        .ok_or("wavファイルパスの変換に失敗しました")?;

    emit_log(&app, "ffmpeg処理を開始します...")?;

    let mut ffmpeg_cmd = Command::new(&ffmpeg_path);
    ffmpeg_cmd.args([
        "-i",
        &file_path,
        "-vn",
        "-ar",
        "16000",
        "-ac",
        "1",
        "-acodec",
        "pcm_s16le",
        "-af",
        "aresample=async=1",
        "-y",
        wav_path_str,
    ]);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        ffmpeg_cmd.creation_flags(0x08000000);
    }

    let ffmpeg_output = ffmpeg_cmd
        .output()
        .map_err(|e| format!("ffmpegの実行に失敗しました: {}", e))?;

    if !ffmpeg_output.status.success() {
        let stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
        return Err(format!("ffmpeg処理に失敗しました: {}", stderr));
    }

    emit_log(&app, "ffmpeg処理が完了しました")?;
    emit_log(&app, "Whisper処理を開始します...")?;

    let out_dir_absolute = if out_dir.is_absolute() {
        out_dir.clone()
    } else {
        env::current_dir()
            .map_err(|e| format!("カレントディレクトリの取得に失敗しました: {}", e))?
            .join(&out_dir)
    };

    let out_dir_str = out_dir_absolute
        .to_str()
        .ok_or("出力ディレクトリパスの変換に失敗しました")?;

    let whisper_args = vec![
        wav_path_str,
        "--model",
        "large-v3",
        "--language",
        "ja",
        "--device",
        "cuda",
        "--compute_type",
        "float16",
        "--vad_filter",
        "True",
        "--output_format",
        "srt",
        "--output_dir",
        out_dir_str,
    ];

    let mut whisper_cmd = Command::new(&whisper_path);
    whisper_cmd.args(&whisper_args);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        whisper_cmd.creation_flags(0x08000000);
    }

    let whisper_output = whisper_cmd
        .output()
        .map_err(|e| format!("Whisperの実行に失敗しました: {}", e))?;

    emit_log(&app, "Whisper処理が完了しました")?;

    let srt_path = out_dir_absolute.join(format!("{}.srt", file_stem));
    let srt_path_str = srt_path
        .to_str()
        .ok_or("SRTファイルパスの変換に失敗しました")?
        .to_string();

    if !srt_path.exists() {
        let stderr = String::from_utf8_lossy(&whisper_output.stderr);
        let stdout = String::from_utf8_lossy(&whisper_output.stdout);
        return Err(format!(
            "SRTファイルが生成されませんでした:\nSTDERR: {}\nSTDOUT: {}",
            stderr, stdout
        ));
    }

    if let Some(width) = max_line_width {
        if width > 0 {
            let content = fs::read_to_string(&srt_path_str)
                .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;
            let adjusted = apply_character_limit_to_content(&content, width as usize)?;
            fs::write(&srt_path_str, adjusted)
                .map_err(|e| format!("SRTファイルの保存に失敗しました: {}", e))?;
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

#[tauri::command]
pub async fn open_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(&path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&path);
        command
    };

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&path);
        command
    };

    command
        .spawn()
        .map_err(|e| format!("パスを開けませんでした: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn read_srt_file(file_path: String) -> Result<Vec<SubtitleEntry>, String> {
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;

    parse_srt(&content)
}

#[tauri::command]
pub async fn save_srt_file(file_path: String, entries: Vec<SubtitleEntry>) -> Result<(), String> {
    fs::write(&file_path, render_srt(&entries))
        .map_err(|e| format!("SRTファイルの保存に失敗しました: {}", e))?;

    Ok(())
}

fn finish_project_job(
    app: &AppHandle,
    job_id: &str,
    result: Result<Project, String>,
    emit_progress: &dyn Fn(JobPhase, JobStatus, String),
) {
    match result {
        Ok(project) => {
            let _ = app.emit("project-updated", project.id);
        }
        Err(error) => {
            let phase = if error.contains("キャンセル") {
                JobPhase::Cancelled
            } else {
                JobPhase::Failed
            };
            let status = if matches!(&phase, JobPhase::Cancelled) {
                JobStatus::Cancelled
            } else {
                JobStatus::Failed
            };
            emit_progress(phase, status, error);
        }
    }

    if let Some(registry) = app.try_state::<JobRegistry>() {
        registry.remove(job_id);
    }
}

fn repository_for(app: &AppHandle) -> Result<ProjectRepository, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("アプリデータディレクトリの取得に失敗しました: {}", e))?;
    Ok(ProjectRepository::new(app_data_dir))
}

fn get_env_var(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("環境変数 {} が設定されていません", key))
}

fn emit_log(app: &AppHandle, message: &str) -> Result<(), String> {
    app.emit("transcription-log", message)
        .map_err(|e| format!("ログの送信に失敗しました: {}", e))
}
