use crate::domain::{
    build_timeline_map, map_subtitles_to_cut_timeline, parse_srt_timestamp_ms, AppSettings,
    AudioMergePreview, AudioMergeTarget, AudioSplitPreview, ClipMarker, DownloadMode,
    DownloadSource, HighlightCandidate, HighlightRequestBundle, HighlightRequestOptions, JobPhase,
    JobProgress, JobStatus, MediaAsset, PreparationOptions, Project, SilenceAnalysis,
    SilenceCutResult, SilenceCutSettings, SilenceSegment, TimelineMap,
};
use crate::infrastructure::{file_name, ProcessRunner, ProcessSpec, Tool, ToolResolver};
use crate::storage::{unix_timestamp, ProjectRepository};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use subtitle_processing::apply_character_limit_to_content;
use uuid::Uuid;

pub const SOCKET_TIMEOUT: &str = "30";
pub const RETRY_COUNT: &str = "3";

pub type ProgressSink<'a> = dyn Fn(JobPhase, JobStatus, String) + Send + Sync + 'a;

pub fn run_preparation_job(
    repository: ProjectRepository,
    project_id: String,
    options: PreparationOptions,
    settings: AppSettings,
    resource_dir: Option<PathBuf>,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<Project, String> {
    progress(
        JobPhase::Queued,
        JobStatus::Running,
        "準備ジョブを開始します".to_string(),
    );

    let mut project = repository.load_project(&project_id)?;
    let resolver = ToolResolver::new(resource_dir, settings);
    let project_dir = PathBuf::from(&project.project_dir);
    fs::create_dir_all(&project_dir)
        .map_err(|e| format!("プロジェクトディレクトリの作成に失敗しました: {}", e))?;

    let media_path = prepare_media(
        &mut project,
        &project_dir,
        &options,
        &resolver,
        runner,
        cancelled.clone(),
        progress,
    )?;

    check_cancelled(&cancelled)?;
    let wav_path = project_dir.join("audio.wav");
    progress(
        JobPhase::ExtractAudio,
        JobStatus::Running,
        "音声抽出を開始します".to_string(),
    );
    runner.run(
        ProcessSpec {
            program: resolver.resolve(Tool::Ffmpeg),
            args: vec![
                "-i".to_string(),
                media_path.to_string_lossy().to_string(),
                "-vn".to_string(),
                "-ar".to_string(),
                "16000".to_string(),
                "-ac".to_string(),
                "1".to_string(),
                "-acodec".to_string(),
                "pcm_s16le".to_string(),
                "-af".to_string(),
                "aresample=async=1".to_string(),
                "-y".to_string(),
                wav_path.to_string_lossy().to_string(),
            ],
            working_dir: Some(project_dir.clone()),
        },
        cancelled.clone(),
        &mut |line| progress(JobPhase::ExtractAudio, JobStatus::Running, line),
    )?;
    project.wav_path = Some(wav_path.to_string_lossy().to_string());

    check_cancelled(&cancelled)?;
    progress(
        JobPhase::Transcribe,
        JobStatus::Running,
        "文字起こしを開始します".to_string(),
    );
    runner.run(
        ProcessSpec {
            program: resolver.resolve(Tool::Whisper),
            args: vec![
                wav_path.to_string_lossy().to_string(),
                "--model".to_string(),
                "large-v3".to_string(),
                "--language".to_string(),
                "ja".to_string(),
                "--device".to_string(),
                "cuda".to_string(),
                "--compute_type".to_string(),
                "float16".to_string(),
                "--vad_filter".to_string(),
                "True".to_string(),
                "--output_format".to_string(),
                "srt".to_string(),
                "--output_dir".to_string(),
                project_dir.to_string_lossy().to_string(),
            ],
            working_dir: Some(project_dir.clone()),
        },
        cancelled.clone(),
        &mut |line| progress(JobPhase::Transcribe, JobStatus::Running, line),
    )?;

    check_cancelled(&cancelled)?;
    progress(
        JobPhase::Postprocess,
        JobStatus::Running,
        "字幕後処理を開始します".to_string(),
    );
    let srt_path = find_generated_srt(&project_dir)?;
    if let Some(width) = options.max_line_width {
        if width > 0 {
            let content = fs::read_to_string(&srt_path)
                .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;
            let adjusted = apply_character_limit_to_content(&content, width as usize)?;
            fs::write(&srt_path, adjusted)
                .map_err(|e| format!("SRTファイルの保存に失敗しました: {}", e))?;
        }
    }

    project.subtitle_path = Some(srt_path.to_string_lossy().to_string());

    if let Some(settings) = &options.silence_cut {
        check_cancelled(&cancelled)?;
        let result = create_silence_cut_for_project(
            &repository,
            &mut project,
            settings.clone(),
            &resolver,
            runner,
            cancelled.clone(),
            progress,
        )?;
        project.silence_cut_path = Some(result.output_path);
        project.timeline_map_path = Some(result.timeline_map_path);
        project.silence_cut_srt_path = Some(result.subtitle_path);
    }

    project.updated_at = unix_timestamp();
    repository.save_project(&project)?;

    progress(
        JobPhase::Completed,
        JobStatus::Succeeded,
        "準備ジョブが完了しました".to_string(),
    );
    Ok(project)
}

fn prepare_media(
    project: &mut Project,
    project_dir: &Path,
    options: &PreparationOptions,
    resolver: &ToolResolver,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<PathBuf, String> {
    match &project.source {
        DownloadSource::LocalFile { path } => {
            let media_path = PathBuf::from(path);
            project.media_asset = Some(MediaAsset {
                path: path.clone(),
                file_name: file_name(&media_path),
                source: project.source.clone(),
            });
            Ok(media_path)
        }
        DownloadSource::Url { url } => {
            check_cancelled(&cancelled)?;
            progress(
                JobPhase::Download,
                JobStatus::Running,
                "動画ダウンロードを開始します".to_string(),
            );
            let output_template = project_dir.join("media.%(ext)s");
            runner.run(
                ProcessSpec {
                    program: resolver.resolve(Tool::YtDlp),
                    args: build_ytdlp_args(url, &output_template, options),
                    working_dir: Some(project_dir.to_path_buf()),
                },
                cancelled,
                &mut |line| progress(JobPhase::Download, JobStatus::Running, line),
            )?;

            let media_path = find_downloaded_media(project_dir)?;
            project.media_asset = Some(MediaAsset {
                file_name: file_name(&media_path),
                path: media_path.to_string_lossy().to_string(),
                source: project.source.clone(),
            });
            Ok(media_path)
        }
    }
}

pub fn build_ytdlp_args(
    url: &str,
    output_template: &Path,
    options: &PreparationOptions,
) -> Vec<String> {
    let mut args = vec![
        url.to_string(),
        "--embed-thumbnail".to_string(),
        "--socket-timeout".to_string(),
        SOCKET_TIMEOUT.to_string(),
        "--ignore-errors".to_string(),
        "--output".to_string(),
        output_template.to_string_lossy().to_string(),
        "--retries".to_string(),
        RETRY_COUNT.to_string(),
        "--newline".to_string(),
        "-S".to_string(),
        "vcodec:h264".to_string(),
        "--merge-output-format".to_string(),
        "mp4".to_string(),
    ];

    match options.download_mode {
        DownloadMode::Video => {
            args.push("-f".to_string());
            args.push("bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best".to_string());
        }
        DownloadMode::Audio => {
            args.push("-f".to_string());
            args.push("bestaudio[ext=m4a]/bestaudio/best".to_string());
            args.push("--extract-audio".to_string());
            args.push("--audio-format".to_string());
            args.push(
                options
                    .audio_format
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "m4a".to_string()),
            );
        }
    }

    args
}

pub fn run_ytdlp_update(
    resolver: &ToolResolver,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let program = resolver.resolve(Tool::YtDlp);
    runner.run(
        ProcessSpec {
            program: program.clone(),
            args: vec!["--rm-cache-dir".to_string()],
            working_dir: None,
        },
        cancelled.clone(),
        progress,
    )?;
    runner.run(
        ProcessSpec {
            program,
            args: vec!["-U".to_string(), "--no-check-certificate".to_string()],
            working_dir: None,
        },
        cancelled,
        progress,
    )
}

pub fn run_media_download_job(
    repository: ProjectRepository,
    project_id: String,
    settings: AppSettings,
    resource_dir: Option<PathBuf>,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<Project, String> {
    progress(
        JobPhase::Queued,
        JobStatus::Running,
        "URL動画取得ジョブを開始します".to_string(),
    );

    let mut project = repository.load_project(&project_id)?;
    if !matches!(project.source, DownloadSource::Url { .. }) {
        return Err("URLプロジェクトではありません".to_string());
    }

    let resolver = ToolResolver::new(resource_dir, settings);
    let project_dir = PathBuf::from(&project.project_dir);
    fs::create_dir_all(&project_dir)
        .map_err(|e| format!("プロジェクトディレクトリの作成に失敗しました: {}", e))?;

    let options = PreparationOptions {
        max_line_width: None,
        download_mode: DownloadMode::Video,
        audio_format: None,
        silence_cut: None,
    };
    let media_path = prepare_media(
        &mut project,
        &project_dir,
        &options,
        &resolver,
        runner,
        cancelled.clone(),
        progress,
    )?;

    check_cancelled(&cancelled)?;
    progress(
        JobPhase::Download,
        JobStatus::Running,
        format!("動画取得が完了しました: {}", media_path.display()),
    );

    project.updated_at = unix_timestamp();
    repository.save_project(&project)?;
    progress(
        JobPhase::Completed,
        JobStatus::Succeeded,
        "URL動画取得ジョブが完了しました".to_string(),
    );
    Ok(project)
}

pub fn build_audio_split_preview_for_project(
    repository: &ProjectRepository,
    project_id: &str,
    split_minutes: u32,
    resolver: &ToolResolver,
) -> Result<AudioSplitPreview, String> {
    if split_minutes == 0 {
        return Err("分割長は1分以上にしてください".to_string());
    }

    let project = repository.load_project(project_id)?;
    let media_path = project_media_path(&project)?;
    let output_dir = audio_split_output_dir(&project, &media_path)?;
    let duration_ms = probe_media_duration_ms(&media_path, resolver)?;
    let split_ms = split_minutes as u64 * 60_000;
    let part_count = duration_ms.div_ceil(split_ms).max(1) as u32;

    Ok(AudioSplitPreview {
        input_path: media_path.to_string_lossy().to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        duration_ms,
        split_minutes,
        part_count,
        output_exists: output_dir.exists(),
    })
}

pub fn run_audio_split_job(
    repository: ProjectRepository,
    project_id: String,
    split_minutes: u32,
    settings: AppSettings,
    resource_dir: Option<PathBuf>,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<Project, String> {
    progress(
        JobPhase::Queued,
        JobStatus::Running,
        "音声分割ジョブを開始します".to_string(),
    );

    let resolver = ToolResolver::new(resource_dir, settings);
    let preview =
        build_audio_split_preview_for_project(&repository, &project_id, split_minutes, &resolver)?;
    if preview.output_exists {
        return Err(format!(
            "分割出力フォルダが既に存在します: {}",
            preview.output_dir
        ));
    }

    let mut project = repository.load_project(&project_id)?;
    let media_path = PathBuf::from(&preview.input_path);
    let output_dir = PathBuf::from(&preview.output_dir);
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("音声分割フォルダの作成に失敗しました: {}", e))?;

    check_cancelled(&cancelled)?;
    progress(
        JobPhase::SplitAudio,
        JobStatus::Running,
        format!(
            "{}分ごとに{}個の24-bit WAVへ分割します",
            split_minutes, preview.part_count
        ),
    );

    runner.run(
        ProcessSpec {
            program: resolver.resolve(Tool::Ffmpeg),
            args: build_audio_split_ffmpeg_args(
                &media_path,
                &output_dir,
                split_minutes as u64 * 60,
            ),
            working_dir: media_path.parent().map(Path::to_path_buf),
        },
        cancelled.clone(),
        &mut |line| progress(JobPhase::SplitAudio, JobStatus::Running, line),
    )?;

    project.audio_split_dir = Some(output_dir.to_string_lossy().to_string());
    project.updated_at = unix_timestamp();
    repository.save_project(&project)?;

    progress(
        JobPhase::Completed,
        JobStatus::Succeeded,
        "音声分割ジョブが完了しました".to_string(),
    );
    Ok(project)
}

pub fn scan_audio_merge_folder(source_dir: &str) -> Result<AudioMergePreview, String> {
    let source_path = PathBuf::from(source_dir);
    if !source_path.is_dir() {
        return Err("WAVフォルダが見つかりません".to_string());
    }

    let mut files = fs::read_dir(&source_path)
        .map_err(|e| format!("WAVフォルダの読み込みに失敗しました: {}", e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        })
        .collect::<Vec<_>>();

    files.sort_by(|left, right| {
        natural_cmp(
            &left
                .file_name()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default(),
            &right
                .file_name()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default(),
        )
    });

    Ok(AudioMergePreview {
        source_dir: source_path.to_string_lossy().to_string(),
        files: files
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
    })
}

pub fn run_audio_merge_job(
    repository: ProjectRepository,
    project_id: String,
    target: AudioMergeTarget,
    source_dir: String,
    settings: AppSettings,
    resource_dir: Option<PathBuf>,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<Project, String> {
    progress(
        JobPhase::Queued,
        JobStatus::Running,
        "WAV結合ジョブを開始します".to_string(),
    );

    let preview = scan_audio_merge_folder(&source_dir)?;
    if preview.files.is_empty() {
        return Err("結合対象のWAVファイルがありません".to_string());
    }

    let mut project = repository.load_project(&project_id)?;
    let project_dir = PathBuf::from(&project.project_dir);
    let merged_dir = project_dir.join("derived").join("audio_merged");
    fs::create_dir_all(&merged_dir)
        .map_err(|e| format!("音声結合フォルダの作成に失敗しました: {}", e))?;

    let (output_path, label) = match target {
        AudioMergeTarget::Vocals => (merged_dir.join("vocals_merged.wav"), "ボーカル"),
        AudioMergeTarget::Bgm => (merged_dir.join("bgm_merged.wav"), "BGM"),
    };
    let concat_path = merged_dir.join(match target {
        AudioMergeTarget::Vocals => "vocals_concat.txt",
        AudioMergeTarget::Bgm => "bgm_concat.txt",
    });
    write_concat_file(&concat_path, &preview.files)?;

    check_cancelled(&cancelled)?;
    progress(
        JobPhase::MergeAudio,
        JobStatus::Running,
        format!("{}WAV {} 件を結合します", label, preview.files.len()),
    );

    let resolver = ToolResolver::new(resource_dir, settings);
    runner.run(
        ProcessSpec {
            program: resolver.resolve(Tool::Ffmpeg),
            args: build_audio_merge_ffmpeg_args(&concat_path, &output_path),
            working_dir: Some(merged_dir.clone()),
        },
        cancelled.clone(),
        &mut |line| progress(JobPhase::MergeAudio, JobStatus::Running, line),
    )?;

    match target {
        AudioMergeTarget::Vocals => {
            project.vocals_source_dir = Some(source_dir);
            project.vocals_merged_path = Some(output_path.to_string_lossy().to_string());
        }
        AudioMergeTarget::Bgm => {
            project.bgm_source_dir = Some(source_dir);
            project.bgm_merged_path = Some(output_path.to_string_lossy().to_string());
        }
    }
    project.updated_at = unix_timestamp();
    repository.save_project(&project)?;

    progress(
        JobPhase::Completed,
        JobStatus::Succeeded,
        "WAV結合ジョブが完了しました".to_string(),
    );
    Ok(project)
}

pub fn run_vocals_transcription_job(
    repository: ProjectRepository,
    project_id: String,
    max_line_width: Option<u32>,
    settings: AppSettings,
    resource_dir: Option<PathBuf>,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<Project, String> {
    progress(
        JobPhase::Queued,
        JobStatus::Running,
        "結合ボーカルから字幕生成を開始します".to_string(),
    );

    let mut project = repository.load_project(&project_id)?;
    let vocals_path = project
        .vocals_merged_path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| "ボーカル結合WAVがまだありません".to_string())?;
    if !vocals_path.exists() {
        return Err("ボーカル結合WAVが見つかりません".to_string());
    }
    let output_dir = PathBuf::from(&project.project_dir)
        .join("derived")
        .join("audio_merged");
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("字幕出力フォルダの作成に失敗しました: {}", e))?;

    check_cancelled(&cancelled)?;
    let resolver = ToolResolver::new(resource_dir, settings);
    progress(
        JobPhase::Transcribe,
        JobStatus::Running,
        "Whisper処理を開始します".to_string(),
    );
    runner.run(
        ProcessSpec {
            program: resolver.resolve(Tool::Whisper),
            args: vec![
                vocals_path.to_string_lossy().to_string(),
                "--model".to_string(),
                "large-v3".to_string(),
                "--language".to_string(),
                "ja".to_string(),
                "--device".to_string(),
                "cuda".to_string(),
                "--compute_type".to_string(),
                "float16".to_string(),
                "--vad_filter".to_string(),
                "True".to_string(),
                "--output_format".to_string(),
                "srt".to_string(),
                "--output_dir".to_string(),
                output_dir.to_string_lossy().to_string(),
            ],
            working_dir: Some(output_dir.clone()),
        },
        cancelled.clone(),
        &mut |line| progress(JobPhase::Transcribe, JobStatus::Running, line),
    )?;

    check_cancelled(&cancelled)?;
    progress(
        JobPhase::Postprocess,
        JobStatus::Running,
        "字幕後処理を開始します".to_string(),
    );
    let srt_path = output_dir.join("vocals_merged.srt");
    if !srt_path.exists() {
        return Err("SRTファイルが生成されませんでした".to_string());
    }
    if let Some(width) = max_line_width {
        if width > 0 {
            let content = fs::read_to_string(&srt_path)
                .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;
            let adjusted = apply_character_limit_to_content(&content, width as usize)?;
            fs::write(&srt_path, adjusted)
                .map_err(|e| format!("SRTファイルの保存に失敗しました: {}", e))?;
        }
    }

    project.wav_path = Some(vocals_path.to_string_lossy().to_string());
    project.subtitle_path = Some(srt_path.to_string_lossy().to_string());
    project.updated_at = unix_timestamp();
    repository.save_project(&project)?;

    progress(
        JobPhase::Completed,
        JobStatus::Succeeded,
        "結合ボーカルからの字幕生成が完了しました".to_string(),
    );
    Ok(project)
}

pub fn detect_silence_for_project(
    repository: &ProjectRepository,
    project_id: &str,
    settings: SilenceCutSettings,
    resolver: &ToolResolver,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<SilenceAnalysis, String> {
    let project = repository.load_project(project_id)?;
    let media_path = project
        .media_asset
        .as_ref()
        .map(|asset| PathBuf::from(&asset.path))
        .or_else(|| match &project.source {
            DownloadSource::LocalFile { path } => Some(PathBuf::from(path)),
            DownloadSource::Url { .. } => None,
        })
        .ok_or_else(|| "無音検出に使う動画ファイルが見つかりません".to_string())?;

    detect_silence(
        &media_path,
        project.subtitle_path.as_deref(),
        settings,
        resolver,
        runner,
        cancelled,
        progress,
    )
}

pub fn create_silence_cut_for_loaded_project(
    repository: &ProjectRepository,
    project_id: &str,
    settings: SilenceCutSettings,
    resolver: &ToolResolver,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<SilenceCutResult, String> {
    let mut project = repository.load_project(project_id)?;
    let result = create_silence_cut_for_project(
        repository,
        &mut project,
        settings,
        resolver,
        runner,
        cancelled,
        progress,
    )?;
    project.silence_cut_path = Some(result.output_path.clone());
    project.timeline_map_path = Some(result.timeline_map_path.clone());
    project.silence_cut_srt_path = Some(result.subtitle_path.clone());
    project.updated_at = unix_timestamp();
    repository.save_project(&project)?;
    Ok(result)
}

fn create_silence_cut_for_project(
    _repository: &ProjectRepository,
    project: &mut Project,
    settings: SilenceCutSettings,
    resolver: &ToolResolver,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<SilenceCutResult, String> {
    let media_path = project
        .media_asset
        .as_ref()
        .map(|asset| PathBuf::from(&asset.path))
        .or_else(|| match &project.source {
            DownloadSource::LocalFile { path } => Some(PathBuf::from(path)),
            DownloadSource::Url { .. } => None,
        })
        .ok_or_else(|| "無音カットに使う動画ファイルが見つかりません".to_string())?;
    let subtitle_path = project
        .subtitle_path
        .as_deref()
        .ok_or_else(|| "無音カット用のSRTファイルがまだありません".to_string())?;

    let analysis = detect_silence(
        &media_path,
        Some(subtitle_path),
        settings,
        resolver,
        runner,
        cancelled.clone(),
        progress,
    )?;

    check_cancelled(&cancelled)?;
    progress(
        JobPhase::CutSilence,
        JobStatus::Running,
        "無音カット済み動画を生成します".to_string(),
    );

    let project_dir = PathBuf::from(&project.project_dir);
    let derived_dir = project_dir.join("derived");
    fs::create_dir_all(&derived_dir)
        .map_err(|e| format!("派生成果物ディレクトリの作成に失敗しました: {}", e))?;

    let output_path = derived_dir.join("silence_cut.mp4");
    let timeline_map = TimelineMap {
        original_duration_ms: analysis.duration_ms,
        cut_duration_ms: analysis
            .keep_segments
            .last()
            .map_or(0, |segment| segment.cut_end_ms),
        segments: analysis.keep_segments.clone(),
    };

    if timeline_map.segments.is_empty() {
        return Err("無音カット後に残る区間がありません".to_string());
    }

    runner.run(
        ProcessSpec {
            program: resolver.resolve(Tool::Ffmpeg),
            args: build_silence_cut_ffmpeg_args(&media_path, &output_path, &timeline_map.segments),
            working_dir: Some(project_dir.clone()),
        },
        cancelled.clone(),
        &mut |line| progress(JobPhase::CutSilence, JobStatus::Running, line),
    )?;

    let timeline_map_path = derived_dir.join("timeline_map.json");
    let timeline_json = serde_json::to_string_pretty(&timeline_map)
        .map_err(|e| format!("タイムライン対応表の変換に失敗しました: {}", e))?;
    fs::write(&timeline_map_path, timeline_json)
        .map_err(|e| format!("タイムライン対応表の保存に失敗しました: {}", e))?;

    let subtitle_content = fs::read_to_string(subtitle_path)
        .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;
    let subtitle_entries = subtitle_processing::parse_srt(&subtitle_content)?;
    let mapped_subtitles = map_subtitles_to_cut_timeline(&subtitle_entries, &timeline_map)?;
    let silence_cut_srt_path = derived_dir.join("silence_cut.srt");
    fs::write(
        &silence_cut_srt_path,
        subtitle_processing::render_srt(&mapped_subtitles),
    )
    .map_err(|e| format!("無音カット用SRTの保存に失敗しました: {}", e))?;

    progress(
        JobPhase::CutSilence,
        JobStatus::Running,
        format!(
            "無音カットが完了しました: {}秒短縮",
            analysis.removed_ms / 1_000
        ),
    );

    Ok(SilenceCutResult {
        output_path: output_path.to_string_lossy().to_string(),
        timeline_map_path: timeline_map_path.to_string_lossy().to_string(),
        subtitle_path: silence_cut_srt_path.to_string_lossy().to_string(),
        analysis,
    })
}

fn detect_silence(
    media_path: &Path,
    subtitle_path: Option<&str>,
    settings: SilenceCutSettings,
    resolver: &ToolResolver,
    runner: &dyn ProcessRunner,
    cancelled: Arc<AtomicBool>,
    progress: &ProgressSink<'_>,
) -> Result<SilenceAnalysis, String> {
    check_cancelled(&cancelled)?;
    progress(
        JobPhase::DetectSilence,
        JobStatus::Running,
        "無音区間の検出を開始します".to_string(),
    );

    let mut output_lines = Vec::new();
    runner.run(
        ProcessSpec {
            program: resolver.resolve(Tool::Ffmpeg),
            args: vec![
                "-i".to_string(),
                media_path.to_string_lossy().to_string(),
                "-af".to_string(),
                format!(
                    "silencedetect=noise={}dB:d={:.3}",
                    settings.noise_db,
                    settings.min_silence_ms as f64 / 1_000.0
                ),
                "-f".to_string(),
                "null".to_string(),
                "-".to_string(),
            ],
            working_dir: media_path.parent().map(Path::to_path_buf),
        },
        cancelled,
        &mut |line| {
            progress(JobPhase::DetectSilence, JobStatus::Running, line.clone());
            output_lines.push(line);
        },
    )?;

    let mut silence_segments = parse_silencedetect_output(&output_lines);
    silence_segments.retain(|segment| segment.end_ms > segment.start_ms);

    let duration_ms = parse_duration_from_output(&output_lines)
        .or_else(|| subtitle_path.and_then(|path| subtitle_duration_ms(path).ok()))
        .ok_or_else(|| {
            "動画の長さを取得できませんでした。SRT生成後にもう一度実行してください".to_string()
        })?;

    let timeline = build_timeline_map(&silence_segments, duration_ms, settings.padding_ms);
    let removed_ms = duration_ms.saturating_sub(timeline.cut_duration_ms);

    progress(
        JobPhase::DetectSilence,
        JobStatus::Running,
        format!(
            "無音区間 {} 件、短縮見込み {} 秒",
            silence_segments.len(),
            removed_ms / 1_000
        ),
    );

    Ok(SilenceAnalysis {
        media_path: media_path.to_string_lossy().to_string(),
        duration_ms,
        settings,
        silence_segments,
        keep_segments: timeline.segments,
        removed_ms,
    })
}

pub fn build_silence_cut_ffmpeg_args(
    input_path: &Path,
    output_path: &Path,
    segments: &[crate::domain::KeepSegment],
) -> Vec<String> {
    let mut filter_parts = Vec::new();
    let mut concat_inputs = String::new();

    for (index, segment) in segments.iter().enumerate() {
        let start = segment.original_start_ms as f64 / 1_000.0;
        let end = segment.original_end_ms as f64 / 1_000.0;
        filter_parts.push(format!(
            "[0:v]trim=start={start:.3}:end={end:.3},setpts=PTS-STARTPTS[v{index}]"
        ));
        filter_parts.push(format!(
            "[0:a]atrim=start={start:.3}:end={end:.3},asetpts=PTS-STARTPTS[a{index}]"
        ));
        concat_inputs.push_str(&format!("[v{index}][a{index}]"));
    }

    filter_parts.push(format!(
        "{}concat=n={}:v=1:a=1[outv][outa]",
        concat_inputs,
        segments.len()
    ));

    vec![
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-filter_complex".to_string(),
        filter_parts.join(";"),
        "-map".to_string(),
        "[outv]".to_string(),
        "-map".to_string(),
        "[outa]".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "veryfast".to_string(),
        "-crf".to_string(),
        "18".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        "-y".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

pub fn generate_highlight_request_for_project(
    repository: &ProjectRepository,
    project_id: &str,
    options: HighlightRequestOptions,
) -> Result<HighlightRequestBundle, String> {
    let mut project = repository.load_project(project_id)?;
    let snapshot = repository.open_project(project_id)?;
    let subtitle_path = snapshot
        .subtitles
        .path
        .clone()
        .ok_or_else(|| "見どころ抽出に使うSRTファイルがまだありません".to_string())?;
    let project_dir = PathBuf::from(&project.project_dir);
    let analysis_dir = project_dir.join("analysis");
    fs::create_dir_all(&analysis_dir)
        .map_err(|e| format!("分析ディレクトリの作成に失敗しました: {}", e))?;

    let request_path = analysis_dir.join("highlight_request.md");
    let schema_path = analysis_dir.join("highlight_candidates.schema.json");
    let candidate_output_path = analysis_dir.join("highlight_candidates.json");

    fs::write(
        &request_path,
        build_highlight_request_markdown(&project, &snapshot.subtitles.entries, &options),
    )
    .map_err(|e| format!("見どころ依頼ファイルの保存に失敗しました: {}", e))?;
    fs::write(&schema_path, highlight_candidates_schema())
        .map_err(|e| format!("見どころ候補スキーマの保存に失敗しました: {}", e))?;

    project.highlight_request_path = Some(request_path.to_string_lossy().to_string());
    project.highlight_candidates_path = Some(candidate_output_path.to_string_lossy().to_string());
    project.updated_at = unix_timestamp();
    repository.save_project(&project)?;

    Ok(HighlightRequestBundle {
        request_path: request_path.to_string_lossy().to_string(),
        schema_path: schema_path.to_string_lossy().to_string(),
        subtitle_path,
        candidate_output_path: candidate_output_path.to_string_lossy().to_string(),
    })
}

pub fn import_highlight_candidates_from_file(file_path: &str) -> Result<Vec<ClipMarker>, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("見どころ候補JSONの読み込みに失敗しました: {}", e))?;
    let candidates = parse_highlight_candidates_json(&content)?;
    candidates
        .into_iter()
        .map(|candidate| candidate.into_marker(Uuid::new_v4().to_string()))
        .collect()
}

fn build_highlight_request_markdown(
    project: &Project,
    entries: &[subtitle_processing::SubtitleEntry],
    options: &HighlightRequestOptions,
) -> String {
    let mut markdown = String::new();
    markdown.push_str("# Movie2Text 見どころ候補抽出依頼\n\n");
    markdown.push_str("以下のSRT字幕だけを材料に、切り抜き候補を抽出してください。\n");
    markdown.push_str("動画そのものは見られない前提なので、字幕から判断できる範囲の候補だけを出してください。\n\n");
    markdown.push_str("## 出力ルール\n");
    markdown.push_str("- JSONのみを出力してください。Markdownの説明文は不要です。\n");
    markdown.push_str("- 形式は `highlight_candidates.schema.json` に合わせてください。\n");
    markdown.push_str("- 候補数の上限は ");
    markdown.push_str(&options.max_candidates.to_string());
    markdown.push_str(" 件です。\n");
    markdown.push_str("- ショート向けに、リアクションが強い箇所、話題転換、オチ、単体で意味が通る発言を優先してください。\n");
    markdown.push_str("- 1候補は原則15秒から90秒程度にしてください。\n");
    markdown.push_str("- `sourceSubtitleIds` には根拠にした字幕番号を入れてください。\n\n");
    markdown.push_str("## Project\n");
    markdown.push_str(&format!("- name: {}\n", project.name));
    markdown.push_str(&format!("- id: {}\n\n", project.id));
    markdown.push_str("## SRT\n\n```srt\n");
    markdown.push_str(&subtitle_processing::render_srt(entries));
    markdown.push_str("```\n");
    markdown
}

fn highlight_candidates_schema() -> String {
    serde_json::json!({
        "type": "object",
        "required": ["candidates"],
        "properties": {
            "candidates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["startMs", "endMs", "title", "reason", "quote", "tags", "sourceSubtitleIds"],
                    "properties": {
                        "startMs": { "type": "integer", "minimum": 0 },
                        "endMs": { "type": "integer", "minimum": 1 },
                        "title": { "type": "string" },
                        "reason": { "type": "string" },
                        "quote": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                        "sourceSubtitleIds": { "type": "array", "items": { "type": "integer" } }
                    }
                }
            }
        }
    })
    .to_string()
}

fn parse_highlight_candidates_json(content: &str) -> Result<Vec<HighlightCandidate>, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CandidateDocument {
        candidates: Vec<HighlightCandidate>,
    }

    serde_json::from_str::<CandidateDocument>(content)
        .map(|document| document.candidates)
        .or_else(|_| serde_json::from_str::<Vec<HighlightCandidate>>(content))
        .map_err(|e| format!("見どころ候補JSONの解析に失敗しました: {}", e))
}

pub fn parse_silencedetect_output(lines: &[String]) -> Vec<SilenceSegment> {
    let mut current_start = None;
    let mut segments = Vec::new();

    for line in lines {
        if let Some(start) = parse_ffmpeg_marker_seconds(line, "silence_start:") {
            current_start = Some(seconds_to_ms(start));
        }

        if let Some(end) = parse_ffmpeg_marker_seconds(line, "silence_end:") {
            if let Some(start_ms) = current_start.take() {
                let end_ms = seconds_to_ms(end);
                if end_ms > start_ms {
                    segments.push(SilenceSegment { start_ms, end_ms });
                }
            }
        }
    }

    segments
}

fn parse_duration_from_output(lines: &[String]) -> Option<u64> {
    lines
        .iter()
        .find_map(|line| parse_ffmpeg_duration_line(line))
}

fn parse_ffmpeg_duration_line(line: &str) -> Option<u64> {
    let marker = "Duration:";
    let start = line.find(marker)? + marker.len();
    let value = line[start..].split(',').next()?.trim();
    parse_colon_time_ms(value)
}

fn parse_colon_time_ms(value: &str) -> Option<u64> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 3 {
        return None;
    }
    let hours = parts[0].parse::<u64>().ok()?;
    let minutes = parts[1].parse::<u64>().ok()?;
    let seconds = parts[2].parse::<f64>().ok()?;
    Some(((hours * 3_600 + minutes * 60) as f64 * 1_000.0 + seconds * 1_000.0).round() as u64)
}

fn parse_ffmpeg_marker_seconds(line: &str, marker: &str) -> Option<f64> {
    let start = line.find(marker)? + marker.len();
    let rest = line[start..].trim_start();
    let value = rest
        .split(|c: char| c.is_whitespace() || c == '|')
        .next()
        .filter(|value| !value.is_empty())?;
    value.parse::<f64>().ok()
}

fn seconds_to_ms(value: f64) -> u64 {
    (value * 1_000.0).round().max(0.0) as u64
}

fn subtitle_duration_ms(path: &str) -> Result<u64, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;
    let entries = subtitle_processing::parse_srt(&content)?;
    entries
        .last()
        .map(|entry| parse_srt_timestamp_ms(&entry.end_time))
        .transpose()?
        .ok_or_else(|| "SRTに字幕エントリがありません".to_string())
}

fn project_media_path(project: &Project) -> Result<PathBuf, String> {
    project
        .media_asset
        .as_ref()
        .map(|asset| PathBuf::from(&asset.path))
        .or_else(|| match &project.source {
            DownloadSource::LocalFile { path } => Some(PathBuf::from(path)),
            DownloadSource::Url { .. } => None,
        })
        .ok_or_else(|| "分割対象の動画ファイルがありません".to_string())
}

fn audio_split_output_dir(project: &Project, media_path: &Path) -> Result<PathBuf, String> {
    let stem = media_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "動画ファイル名の取得に失敗しました".to_string())?;
    Ok(PathBuf::from(&project.project_dir)
        .join("derived")
        .join("audio_splits")
        .join(stem))
}

fn probe_media_duration_ms(media_path: &Path, resolver: &ToolResolver) -> Result<u64, String> {
    let ffprobe_output = command_output(
        &resolver.resolve_ffprobe(),
        &[
            "-v".to_string(),
            "error".to_string(),
            "-show_entries".to_string(),
            "format=duration".to_string(),
            "-of".to_string(),
            "default=noprint_wrappers=1:nokey=1".to_string(),
            media_path.to_string_lossy().to_string(),
        ],
    );

    if let Ok(output) = ffprobe_output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(seconds) = stdout.trim().parse::<f64>() {
                return Ok((seconds * 1_000.0).round() as u64);
            }
        }
    }

    let output = command_output(
        &resolver.resolve(Tool::Ffmpeg),
        &["-i".to_string(), media_path.to_string_lossy().to_string()],
    )
    .map_err(|e| format!("動画尺の取得に失敗しました: {}", e))?;
    let mut lines = String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.extend(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string),
    );
    parse_duration_from_output(&lines).ok_or_else(|| "動画尺を取得できませんでした".to_string())
}

fn command_output(program: &str, args: &[String]) -> Result<std::process::Output, String> {
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .output()
        .map_err(|e| format!("外部プロセスの実行に失敗しました: {}", e))
}

pub fn build_audio_split_ffmpeg_args(
    input_path: &Path,
    output_dir: &Path,
    split_seconds: u64,
) -> Vec<String> {
    vec![
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-map".to_string(),
        "0:a:0".to_string(),
        "-vn".to_string(),
        "-c:a".to_string(),
        "pcm_s24le".to_string(),
        "-f".to_string(),
        "segment".to_string(),
        "-segment_time".to_string(),
        split_seconds.to_string(),
        "-segment_start_number".to_string(),
        "1".to_string(),
        "-reset_timestamps".to_string(),
        "1".to_string(),
        "-y".to_string(),
        output_dir
            .join("part_%03d.wav")
            .to_string_lossy()
            .to_string(),
    ]
}

pub fn build_audio_merge_ffmpeg_args(concat_path: &Path, output_path: &Path) -> Vec<String> {
    vec![
        "-f".to_string(),
        "concat".to_string(),
        "-safe".to_string(),
        "0".to_string(),
        "-i".to_string(),
        concat_path.to_string_lossy().to_string(),
        "-c:a".to_string(),
        "pcm_s24le".to_string(),
        "-y".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

fn write_concat_file(path: &Path, files: &[String]) -> Result<(), String> {
    let mut file =
        fs::File::create(path).map_err(|e| format!("concatファイルの作成に失敗しました: {}", e))?;
    for item in files {
        let escaped = item.replace('\\', "/").replace('\'', "'\\''");
        writeln!(file, "file '{}'", escaped)
            .map_err(|e| format!("concatファイルの書き込みに失敗しました: {}", e))?;
    }
    Ok(())
}

fn natural_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    let mut left_iter = left.chars().peekable();
    let mut right_iter = right.chars().peekable();

    loop {
        match (left_iter.peek(), right_iter.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(left_char), Some(right_char)) => {
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() {
                    let left_number = take_number(&mut left_iter);
                    let right_number = take_number(&mut right_iter);
                    let ordering = left_number.cmp(&right_number);
                    if !ordering.is_eq() {
                        return ordering;
                    }
                    continue;
                }

                let left_char = left_iter.next().unwrap().to_ascii_lowercase();
                let right_char = right_iter.next().unwrap().to_ascii_lowercase();
                let ordering = left_char.cmp(&right_char);
                if !ordering.is_eq() {
                    return ordering;
                }
            }
        }
    }
}

fn take_number(iter: &mut std::iter::Peekable<std::str::Chars<'_>>) -> u64 {
    let mut value = String::new();
    while iter.peek().is_some_and(|char| char.is_ascii_digit()) {
        value.push(iter.next().unwrap());
    }
    value.parse::<u64>().unwrap_or(0)
}

fn find_downloaded_media(project_dir: &Path) -> Result<PathBuf, String> {
    fs::read_dir(project_dir)
        .map_err(|e| format!("ダウンロード結果の確認に失敗しました: {}", e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with("media."))
        })
        .ok_or_else(|| "ダウンロードされた素材ファイルが見つかりません".to_string())
}

fn find_generated_srt(project_dir: &Path) -> Result<PathBuf, String> {
    let preferred = project_dir.join("audio.srt");
    if preferred.exists() {
        return Ok(preferred);
    }

    fs::read_dir(project_dir)
        .map_err(|e| format!("SRT生成結果の確認に失敗しました: {}", e))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("srt"))
        .ok_or_else(|| "SRTファイルが生成されませんでした".to_string())
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::SeqCst) {
        Err("ジョブはキャンセルされました".to_string())
    } else {
        Ok(())
    }
}

pub fn progress_payload(
    job_id: &str,
    project_id: &str,
    phase: JobPhase,
    status: JobStatus,
    message: String,
) -> JobProgress {
    JobProgress {
        job_id: job_id.to_string(),
        project_id: project_id.to_string(),
        phase,
        status,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::tests::FakeRunner;
    use std::sync::Mutex;

    #[test]
    fn video_download_args_match_ytdlgui_behavior() {
        let args = build_ytdlp_args(
            "https://example.com/watch",
            Path::new("media.%(ext)s"),
            &PreparationOptions {
                max_line_width: Some(23),
                download_mode: DownloadMode::Video,
                audio_format: None,
                silence_cut: None,
            },
        );

        assert!(args.contains(&"--embed-thumbnail".to_string()));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-S" && pair[1] == "vcodec:h264"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--merge-output-format" && pair[1] == "mp4"));
        assert!(args.iter().any(|arg| arg.contains("bestvideo[ext=mp4]")));
    }

    #[test]
    fn audio_download_args_extract_audio() {
        let args = build_ytdlp_args(
            "https://example.com/watch",
            Path::new("media.%(ext)s"),
            &PreparationOptions {
                max_line_width: Some(23),
                download_mode: DownloadMode::Audio,
                audio_format: Some("mp3".to_string()),
                silence_cut: None,
            },
        );

        assert!(args.contains(&"--extract-audio".to_string()));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--audio-format" && pair[1] == "mp3"));
    }

    #[test]
    fn silencedetect_output_is_parsed_into_segments() {
        let lines = vec![
            "Duration: 00:00:12.50, start: 0.000000, bitrate: 1000 kb/s".to_string(),
            "[silencedetect @ 000] silence_start: 1.25".to_string(),
            "[silencedetect @ 000] silence_end: 2.5 | silence_duration: 1.25".to_string(),
        ];

        assert_eq!(
            parse_silencedetect_output(&lines),
            vec![SilenceSegment {
                start_ms: 1_250,
                end_ms: 2_500,
            }]
        );
        assert_eq!(parse_duration_from_output(&lines), Some(12_500));
    }

    #[test]
    fn silence_cut_args_use_trim_concat_filter() {
        let args = build_silence_cut_ffmpeg_args(
            Path::new("input.mp4"),
            Path::new("output.mp4"),
            &[crate::domain::KeepSegment {
                original_start_ms: 0,
                original_end_ms: 2_000,
                cut_start_ms: 0,
                cut_end_ms: 2_000,
            }],
        );

        assert!(args
            .iter()
            .any(|arg| arg.contains("trim=start=0.000:end=2.000")));
        assert!(args.contains(&"libx264".to_string()));
    }

    #[test]
    fn highlight_candidates_json_accepts_wrapped_document() {
        let markers = parse_highlight_candidates_json(
            r#"{
                "candidates": [
                    {
                        "startMs": 1000,
                        "endMs": 5000,
                        "title": "山場",
                        "reason": "反応が強い",
                        "quote": "えっ",
                        "tags": ["short"],
                        "confidence": 0.9,
                        "sourceSubtitleIds": [1]
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].title, "山場");
    }

    #[test]
    fn silence_cut_job_writes_timeline_and_cut_srt_with_fake_process() {
        let temp = tempfile::tempdir().unwrap();
        let repository = ProjectRepository::new(temp.path().join("app"));
        let media_path = temp.path().join("video.mp4");
        fs::write(&media_path, b"fake").unwrap();
        let project = repository
            .create_project(
                "テスト".to_string(),
                DownloadSource::LocalFile {
                    path: media_path.to_string_lossy().to_string(),
                },
            )
            .unwrap();
        repository
            .save_subtitles(
                &project.id,
                &[subtitle_processing::SubtitleEntry {
                    index: 1,
                    start_time: "00:00:00,000".to_string(),
                    end_time: "00:00:04,000".to_string(),
                    text: "テスト".to_string(),
                }],
            )
            .unwrap();

        let runner = SilenceFakeRunner::default();
        let resolver = ToolResolver::new(None, AppSettings::default());
        let result = create_silence_cut_for_loaded_project(
            &repository,
            &project.id,
            SilenceCutSettings {
                noise_db: -35,
                min_silence_ms: 400,
                padding_ms: 0,
            },
            &resolver,
            &runner,
            Arc::new(AtomicBool::new(false)),
            &|_, _, _| {},
        )
        .unwrap();

        assert!(Path::new(&result.timeline_map_path).exists());
        assert!(Path::new(&result.subtitle_path).exists());
        assert_eq!(result.analysis.keep_segments.len(), 2);
        assert_eq!(runner.calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn fake_runner_is_still_available_for_existing_runner_tests() {
        let runner = FakeRunner::default();
        assert!(runner.programs.lock().unwrap().is_empty());
    }

    #[derive(Default)]
    struct SilenceFakeRunner {
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl ProcessRunner for SilenceFakeRunner {
        fn run(
            &self,
            spec: ProcessSpec,
            _cancelled: Arc<AtomicBool>,
            on_output: &mut dyn FnMut(String),
        ) -> Result<(), String> {
            let mut calls = self.calls.lock().unwrap();
            calls.push(spec.args.clone());
            if calls.len() == 1 {
                on_output("Duration: 00:00:04.00, start: 0.000000, bitrate: 1000 kb/s".to_string());
                on_output("[silencedetect @ 000] silence_start: 1".to_string());
                on_output("[silencedetect @ 000] silence_end: 2 | silence_duration: 1".to_string());
            }
            Ok(())
        }
    }
}
