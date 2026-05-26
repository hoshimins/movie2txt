use serde::{Deserialize, Serialize};
use subtitle_processing::SubtitleEntry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DownloadSource {
    Url { url: String },
    LocalFile { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadMode {
    Video,
    Audio,
}

impl Default for DownloadMode {
    fn default() -> Self {
        Self::Video
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaAsset {
    pub path: String,
    pub file_name: String,
    pub source: DownloadSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipMarker {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub title: String,
    pub memo: String,
    pub tags: Vec<String>,
    pub source_subtitle_ids: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleDocument {
    pub path: Option<String>,
    pub entries: Vec<SubtitleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub source: DownloadSource,
    pub project_dir: String,
    pub media_asset: Option<MediaAsset>,
    pub wav_path: Option<String>,
    pub subtitle_path: Option<String>,
    #[serde(default)]
    pub silence_cut_path: Option<String>,
    #[serde(default)]
    pub timeline_map_path: Option<String>,
    #[serde(default)]
    pub silence_cut_srt_path: Option<String>,
    #[serde(default)]
    pub highlight_request_path: Option<String>,
    #[serde(default)]
    pub highlight_candidates_path: Option<String>,
    pub markers: Vec<ClipMarker>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub source: DownloadSource,
    pub media_file_name: Option<String>,
    pub subtitle_ready: bool,
    pub marker_count: usize,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub project: Project,
    pub subtitles: SubtitleDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreparationOptions {
    #[serde(default)]
    pub max_line_width: Option<u32>,
    #[serde(default)]
    pub download_mode: DownloadMode,
    #[serde(default)]
    pub audio_format: Option<String>,
    #[serde(default)]
    pub silence_cut: Option<SilenceCutSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SilenceCutSettings {
    pub noise_db: i32,
    pub min_silence_ms: u64,
    pub padding_ms: u64,
}

impl Default for SilenceCutSettings {
    fn default() -> Self {
        Self {
            noise_db: -35,
            min_silence_ms: 400,
            padding_ms: 150,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SilenceSegment {
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KeepSegment {
    pub original_start_ms: u64,
    pub original_end_ms: u64,
    pub cut_start_ms: u64,
    pub cut_end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineMap {
    pub original_duration_ms: u64,
    pub cut_duration_ms: u64,
    pub segments: Vec<KeepSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SilenceAnalysis {
    pub media_path: String,
    pub duration_ms: u64,
    pub settings: SilenceCutSettings,
    pub silence_segments: Vec<SilenceSegment>,
    pub keep_segments: Vec<KeepSegment>,
    pub removed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SilenceCutResult {
    pub output_path: String,
    pub timeline_map_path: String,
    pub subtitle_path: String,
    pub analysis: SilenceAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HighlightRequestOptions {
    #[serde(default = "default_highlight_candidate_count")]
    pub max_candidates: u32,
}

impl Default for HighlightRequestOptions {
    fn default() -> Self {
        Self {
            max_candidates: default_highlight_candidate_count(),
        }
    }
}

fn default_highlight_candidate_count() -> u32 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HighlightRequestBundle {
    pub request_path: String,
    pub schema_path: String,
    pub subtitle_path: String,
    pub candidate_output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HighlightCandidate {
    pub start_ms: u64,
    pub end_ms: u64,
    pub title: String,
    pub reason: String,
    pub quote: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub source_subtitle_ids: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub ytdlp_path: Option<String>,
    #[serde(default)]
    pub ffmpeg_path: Option<String>,
    #[serde(default)]
    pub whisper_path: Option<String>,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub tmp_dir: Option<String>,
    #[serde(default = "default_max_line_width")]
    pub max_line_width: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ytdlp_path: None,
            ffmpeg_path: None,
            whisper_path: None,
            output_dir: None,
            tmp_dir: None,
            max_line_width: default_max_line_width(),
        }
    }
}

fn default_max_line_width() -> u32 {
    23
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JobId {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub job_id: String,
    pub project_id: String,
    pub phase: JobPhase,
    pub status: JobStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum JobPhase {
    Queued,
    Download,
    ExtractAudio,
    Transcribe,
    Postprocess,
    DetectSilence,
    CutSilence,
    GenerateHighlightRequest,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum JobStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

pub fn project_summary(project: &Project) -> ProjectSummary {
    ProjectSummary {
        id: project.id.clone(),
        name: project.name.clone(),
        source: project.source.clone(),
        media_file_name: project
            .media_asset
            .as_ref()
            .map(|asset| asset.file_name.clone()),
        subtitle_ready: project.subtitle_path.is_some(),
        marker_count: project.markers.len(),
        updated_at: project.updated_at,
    }
}

pub fn validate_marker(marker: &ClipMarker) -> Result<(), String> {
    if marker.end_ms <= marker.start_ms {
        return Err("切り抜き候補の終了時刻は開始時刻より後にしてください".to_string());
    }

    if marker.title.trim().is_empty() {
        return Err("切り抜き候補のタイトルを入力してください".to_string());
    }

    Ok(())
}

pub fn build_timeline_map(
    silence_segments: &[SilenceSegment],
    duration_ms: u64,
    padding_ms: u64,
) -> TimelineMap {
    let mut removal_segments = silence_segments
        .iter()
        .filter_map(|segment| {
            let start = segment.start_ms.saturating_add(padding_ms).min(duration_ms);
            let end = segment.end_ms.saturating_sub(padding_ms).min(duration_ms);
            (end > start).then_some((start, end))
        })
        .collect::<Vec<_>>();

    removal_segments.sort_by_key(|segment| segment.0);

    let mut merged = Vec::<(u64, u64)>::new();
    for (start, end) in removal_segments {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let mut keep_segments = Vec::new();
    let mut cursor = 0;
    let mut cut_cursor = 0;
    for (remove_start, remove_end) in merged {
        if remove_start > cursor {
            let cut_end = cut_cursor + (remove_start - cursor);
            keep_segments.push(KeepSegment {
                original_start_ms: cursor,
                original_end_ms: remove_start,
                cut_start_ms: cut_cursor,
                cut_end_ms: cut_end,
            });
            cut_cursor = cut_end;
        }
        cursor = cursor.max(remove_end);
    }

    if cursor < duration_ms {
        keep_segments.push(KeepSegment {
            original_start_ms: cursor,
            original_end_ms: duration_ms,
            cut_start_ms: cut_cursor,
            cut_end_ms: cut_cursor + (duration_ms - cursor),
        });
    }

    TimelineMap {
        original_duration_ms: duration_ms,
        cut_duration_ms: keep_segments.last().map_or(0, |segment| segment.cut_end_ms),
        segments: keep_segments,
    }
}

impl TimelineMap {
    pub fn original_to_cut_ms(&self, original_ms: u64) -> Option<u64> {
        self.segments
            .iter()
            .find(|segment| {
                original_ms >= segment.original_start_ms && original_ms <= segment.original_end_ms
            })
            .map(|segment| {
                segment.cut_start_ms + original_ms.saturating_sub(segment.original_start_ms)
            })
    }

    pub fn nearest_cut_ms(&self, original_ms: u64) -> Option<u64> {
        self.original_to_cut_ms(original_ms).or_else(|| {
            self.segments
                .iter()
                .find(|segment| original_ms < segment.original_start_ms)
                .map(|segment| segment.cut_start_ms)
                .or_else(|| self.segments.last().map(|segment| segment.cut_end_ms))
        })
    }
}

pub fn map_subtitles_to_cut_timeline(
    entries: &[SubtitleEntry],
    timeline_map: &TimelineMap,
) -> Result<Vec<SubtitleEntry>, String> {
    let mut mapped = Vec::new();

    for entry in entries {
        let start_ms = parse_srt_timestamp_ms(&entry.start_time)?;
        let end_ms = parse_srt_timestamp_ms(&entry.end_time)?;
        let Some(mapped_start) = timeline_map.nearest_cut_ms(start_ms) else {
            continue;
        };
        let Some(mut mapped_end) = timeline_map.nearest_cut_ms(end_ms) else {
            continue;
        };

        if mapped_end <= mapped_start {
            mapped_end = (mapped_start + 600).min(timeline_map.cut_duration_ms);
        }

        if mapped_end <= mapped_start {
            continue;
        }

        mapped.push(SubtitleEntry {
            index: mapped.len() + 1,
            start_time: format_srt_timestamp_ms(mapped_start),
            end_time: format_srt_timestamp_ms(mapped_end),
            text: entry.text.clone(),
        });
    }

    Ok(mapped)
}

pub fn parse_srt_timestamp_ms(value: &str) -> Result<u64, String> {
    let (time, millis) = value
        .split_once(',')
        .ok_or_else(|| format!("SRT時刻の形式が不正です: {}", value))?;
    let mut parts = time.split(':');
    let hours = parts
        .next()
        .ok_or_else(|| format!("SRT時刻の形式が不正です: {}", value))?
        .parse::<u64>()
        .map_err(|_| format!("SRT時刻の時が不正です: {}", value))?;
    let minutes = parts
        .next()
        .ok_or_else(|| format!("SRT時刻の形式が不正です: {}", value))?
        .parse::<u64>()
        .map_err(|_| format!("SRT時刻の分が不正です: {}", value))?;
    let seconds = parts
        .next()
        .ok_or_else(|| format!("SRT時刻の形式が不正です: {}", value))?
        .parse::<u64>()
        .map_err(|_| format!("SRT時刻の秒が不正です: {}", value))?;
    let millis = millis
        .parse::<u64>()
        .map_err(|_| format!("SRT時刻のミリ秒が不正です: {}", value))?;

    Ok(((hours * 60 + minutes) * 60 + seconds) * 1_000 + millis)
}

pub fn format_srt_timestamp_ms(value: u64) -> String {
    let total_seconds = value / 1_000;
    let millis = value % 1_000;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    let minutes = total_minutes % 60;
    let hours = total_minutes / 60;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

impl HighlightCandidate {
    pub fn into_marker(self, id: String) -> Result<ClipMarker, String> {
        let title = self.title.trim().to_string();
        let marker = ClipMarker {
            id,
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            title: if title.is_empty() {
                "見どころ候補".to_string()
            } else {
                title
            },
            memo: build_candidate_memo(&self),
            tags: self.tags,
            source_subtitle_ids: self.source_subtitle_ids,
        };
        validate_marker(&marker)?;
        Ok(marker)
    }
}

fn build_candidate_memo(candidate: &HighlightCandidate) -> String {
    let mut lines = Vec::new();
    if !candidate.reason.trim().is_empty() {
        lines.push(format!("理由: {}", candidate.reason.trim()));
    }
    if !candidate.quote.trim().is_empty() {
        lines.push(format!("引用: {}", candidate.quote.trim()));
    }
    if let Some(confidence) = candidate.confidence {
        lines.push(format!("信頼度: {:.2}", confidence));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_requires_positive_range() {
        let marker = ClipMarker {
            id: "marker-1".to_string(),
            start_ms: 1_000,
            end_ms: 1_000,
            title: "候補".to_string(),
            memo: String::new(),
            tags: Vec::new(),
            source_subtitle_ids: Vec::new(),
        };

        assert!(validate_marker(&marker).is_err());
    }

    #[test]
    fn marker_requires_title() {
        let marker = ClipMarker {
            id: "marker-1".to_string(),
            start_ms: 1_000,
            end_ms: 2_000,
            title: " ".to_string(),
            memo: String::new(),
            tags: Vec::new(),
            source_subtitle_ids: Vec::new(),
        };

        assert!(validate_marker(&marker).is_err());
    }

    #[test]
    fn timeline_map_keeps_non_silent_ranges_with_padding() {
        let map = build_timeline_map(
            &[SilenceSegment {
                start_ms: 1_000,
                end_ms: 2_000,
            }],
            4_000,
            150,
        );

        assert_eq!(
            map.segments,
            vec![
                KeepSegment {
                    original_start_ms: 0,
                    original_end_ms: 1_150,
                    cut_start_ms: 0,
                    cut_end_ms: 1_150,
                },
                KeepSegment {
                    original_start_ms: 1_850,
                    original_end_ms: 4_000,
                    cut_start_ms: 1_150,
                    cut_end_ms: 3_300,
                },
            ]
        );
        assert_eq!(map.cut_duration_ms, 3_300);
    }

    #[test]
    fn timeline_map_padding_never_creates_negative_ranges() {
        let map = build_timeline_map(
            &[SilenceSegment {
                start_ms: 100,
                end_ms: 200,
            }],
            1_000,
            150,
        );

        assert_eq!(map.segments.len(), 1);
        assert_eq!(map.segments[0].original_start_ms, 0);
        assert_eq!(map.segments[0].original_end_ms, 1_000);
    }

    #[test]
    fn timeline_map_converts_original_time_to_cut_time() {
        let map = build_timeline_map(
            &[SilenceSegment {
                start_ms: 1_000,
                end_ms: 2_000,
            }],
            4_000,
            0,
        );

        assert_eq!(map.original_to_cut_ms(500), Some(500));
        assert_eq!(map.original_to_cut_ms(2_500), Some(1_500));
        assert_eq!(map.original_to_cut_ms(1_500), None);
    }

    #[test]
    fn highlight_candidate_converts_to_marker() {
        let marker = HighlightCandidate {
            start_ms: 1_000,
            end_ms: 3_000,
            title: "見どころ".to_string(),
            reason: "反応が強い".to_string(),
            quote: "すごい".to_string(),
            tags: vec!["short".to_string()],
            confidence: Some(0.8),
            source_subtitle_ids: vec![1, 2],
        }
        .into_marker("candidate-1".to_string())
        .unwrap();

        assert_eq!(marker.title, "見どころ");
        assert!(marker.memo.contains("反応が強い"));
        assert_eq!(marker.source_subtitle_ids, vec![1, 2]);
    }
}
