use crate::domain::{
    project_summary, AppSettings, DownloadSource, Project, ProjectSnapshot, ProjectSummary,
    SubtitleDocument,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use subtitle_processing::{parse_srt, SubtitleEntry};
use uuid::Uuid;

const PROJECT_FILE: &str = "project.json";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone)]
pub struct ProjectRepository {
    root_dir: PathBuf,
}

impl ProjectRepository {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn create_project(&self, name: String, source: DownloadSource) -> Result<Project, String> {
        fs::create_dir_all(self.projects_dir())
            .map_err(|e| format!("プロジェクト保存ディレクトリの作成に失敗しました: {}", e))?;

        let id = Uuid::new_v4().to_string();
        let project_dir = self.projects_dir().join(&id);
        fs::create_dir_all(&project_dir)
            .map_err(|e| format!("プロジェクトディレクトリの作成に失敗しました: {}", e))?;

        let timestamp = unix_timestamp();
        let project = Project {
            id,
            name,
            source,
            project_dir: project_dir.to_string_lossy().to_string(),
            media_asset: None,
            wav_path: None,
            subtitle_path: None,
            silence_cut_path: None,
            timeline_map_path: None,
            silence_cut_srt_path: None,
            highlight_request_path: None,
            highlight_candidates_path: None,
            markers: Vec::new(),
            created_at: timestamp,
            updated_at: timestamp,
        };

        self.save_project(&project)?;
        Ok(project)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>, String> {
        let projects_dir = self.projects_dir();
        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut projects = fs::read_dir(&projects_dir)
            .map_err(|e| format!("プロジェクト一覧の読み込みに失敗しました: {}", e))?
            .filter_map(Result::ok)
            .filter_map(|entry| self.read_project_file(entry.path().join(PROJECT_FILE)).ok())
            .map(|project| project_summary(&project))
            .collect::<Vec<_>>();

        projects.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(projects)
    }

    pub fn open_project(&self, project_id: &str) -> Result<ProjectSnapshot, String> {
        let project = self.load_project(project_id)?;
        let subtitles = match &project.subtitle_path {
            Some(path) => self.read_subtitle_document(path)?,
            None => SubtitleDocument {
                path: None,
                entries: Vec::new(),
            },
        };

        Ok(ProjectSnapshot { project, subtitles })
    }

    pub fn load_project(&self, project_id: &str) -> Result<Project, String> {
        self.read_project_file(self.project_file(project_id))
    }

    pub fn save_project(&self, project: &Project) -> Result<(), String> {
        let project_dir = PathBuf::from(&project.project_dir);
        fs::create_dir_all(&project_dir)
            .map_err(|e| format!("プロジェクトディレクトリの作成に失敗しました: {}", e))?;

        let json = serde_json::to_string_pretty(project)
            .map_err(|e| format!("プロジェクト情報の変換に失敗しました: {}", e))?;
        fs::write(project_dir.join(PROJECT_FILE), json)
            .map_err(|e| format!("プロジェクト情報の保存に失敗しました: {}", e))
    }

    pub fn save_subtitles(
        &self,
        project_id: &str,
        entries: &[SubtitleEntry],
    ) -> Result<Project, String> {
        let mut project = self.load_project(project_id)?;
        let path = project.subtitle_path.clone().unwrap_or_else(|| {
            PathBuf::from(&project.project_dir)
                .join("audio.srt")
                .to_string_lossy()
                .to_string()
        });
        fs::write(&path, subtitle_processing::render_srt(entries))
            .map_err(|e| format!("SRTファイルの保存に失敗しました: {}", e))?;
        project.subtitle_path = Some(path);
        project.updated_at = unix_timestamp();
        self.save_project(&project)?;
        Ok(project)
    }

    pub fn save_markers(
        &self,
        project_id: &str,
        markers: Vec<crate::domain::ClipMarker>,
    ) -> Result<Project, String> {
        for marker in &markers {
            crate::domain::validate_marker(marker)?;
        }

        let mut project = self.load_project(project_id)?;
        project.markers = markers;
        project.updated_at = unix_timestamp();
        self.save_project(&project)?;
        Ok(project)
    }

    pub fn load_settings(&self) -> Result<AppSettings, String> {
        let path = self.root_dir.join(SETTINGS_FILE);
        if !path.exists() {
            return Ok(AppSettings::default());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| format!("設定ファイルの読み込みに失敗しました: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("設定ファイルの解析に失敗しました: {}", e))
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        fs::create_dir_all(&self.root_dir)
            .map_err(|e| format!("設定ディレクトリの作成に失敗しました: {}", e))?;
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("設定の変換に失敗しました: {}", e))?;
        fs::write(self.root_dir.join(SETTINGS_FILE), json)
            .map_err(|e| format!("設定ファイルの保存に失敗しました: {}", e))
    }

    fn projects_dir(&self) -> PathBuf {
        self.root_dir.join("projects")
    }

    fn project_file(&self, project_id: &str) -> PathBuf {
        self.projects_dir().join(project_id).join(PROJECT_FILE)
    }

    fn read_project_file(&self, path: PathBuf) -> Result<Project, String> {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("プロジェクト情報の読み込みに失敗しました: {}", e))?;
        serde_json::from_str(&content).map_err(|e| {
            format!(
                "プロジェクト情報の解析に失敗しました ({}): {}",
                path.display(),
                e
            )
        })
    }

    fn read_subtitle_document(&self, path: &str) -> Result<SubtitleDocument, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("SRTファイルの読み込みに失敗しました: {}", e))?;
        Ok(SubtitleDocument {
            path: Some(path.to_string()),
            entries: parse_srt(&content)?,
        })
    }
}

pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ClipMarker;

    #[test]
    fn project_can_be_created_and_resumed() {
        let temp = tempfile::tempdir().unwrap();
        let repository = ProjectRepository::new(temp.path().to_path_buf());

        let project = repository
            .create_project(
                "テスト".to_string(),
                DownloadSource::Url {
                    url: "https://example.com/watch".to_string(),
                },
            )
            .unwrap();

        let snapshot = repository.open_project(&project.id).unwrap();
        assert_eq!(snapshot.project.name, "テスト");
        assert!(snapshot.subtitles.entries.is_empty());
    }

    #[test]
    fn markers_are_saved_with_project() {
        let temp = tempfile::tempdir().unwrap();
        let repository = ProjectRepository::new(temp.path().to_path_buf());
        let project = repository
            .create_project(
                "テスト".to_string(),
                DownloadSource::LocalFile {
                    path: "video.mp4".to_string(),
                },
            )
            .unwrap();

        repository
            .save_markers(
                &project.id,
                vec![ClipMarker {
                    id: "marker-1".to_string(),
                    start_ms: 1_000,
                    end_ms: 2_000,
                    title: "候補".to_string(),
                    memo: "メモ".to_string(),
                    tags: vec!["short".to_string()],
                    source_subtitle_ids: vec![1],
                }],
            )
            .unwrap();

        let snapshot = repository.open_project(&project.id).unwrap();
        assert_eq!(snapshot.project.markers.len(), 1);
    }
}
