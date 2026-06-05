use crate::domain::AppSettings;
use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    YtDlp,
    Ffmpeg,
    Whisper,
}

impl Tool {
    fn env_key(self) -> &'static str {
        match self {
            Self::YtDlp => "YTDLP_PATH",
            Self::Ffmpeg => "FFMPEG_PATH",
            Self::Whisper => "WHISPER_PATH",
        }
    }

    fn command_name(self) -> &'static str {
        match self {
            Self::YtDlp => "yt-dlp",
            Self::Ffmpeg => "ffmpeg",
            Self::Whisper => "faster-whisper",
        }
    }

    fn windows_file_name(self) -> &'static str {
        match self {
            Self::YtDlp => "yt-dlp.exe",
            Self::Ffmpeg => "ffmpeg.exe",
            Self::Whisper => "faster-whisper.exe",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolResolver {
    resource_dir: Option<PathBuf>,
    settings: AppSettings,
}

impl ToolResolver {
    pub fn new(resource_dir: Option<PathBuf>, settings: AppSettings) -> Self {
        Self {
            resource_dir,
            settings,
        }
    }

    pub fn resolve(&self, tool: Tool) -> String {
        let configured = match tool {
            Tool::YtDlp => self.settings.ytdlp_path.as_deref(),
            Tool::Ffmpeg => self.settings.ffmpeg_path.as_deref(),
            Tool::Whisper => self.settings.whisper_path.as_deref(),
        };

        if let Some(path) = configured {
            if !path.trim().is_empty() {
                return path.to_string();
            }
        }

        if let Ok(path) = env::var(tool.env_key()) {
            if !path.trim().is_empty() {
                return path;
            }
        }

        if let Some(resource_dir) = &self.resource_dir {
            let bundled = resource_dir.join("binaries").join(tool.windows_file_name());
            if bundled.exists() {
                return bundled.to_string_lossy().to_string();
            }
        }

        tool.command_name().to_string()
    }

    pub fn resolve_ffprobe(&self) -> String {
        let ffmpeg = self.resolve(Tool::Ffmpeg);
        let ffmpeg_path = PathBuf::from(&ffmpeg);
        let ffprobe_file = if cfg!(target_os = "windows") {
            "ffprobe.exe"
        } else {
            "ffprobe"
        };

        if ffmpeg_path.file_name().is_some() {
            if let Some(parent) = ffmpeg_path.parent() {
                let sibling = parent.join(ffprobe_file);
                if sibling.exists() {
                    return sibling.to_string_lossy().to_string();
                }
            }
        }

        "ffprobe".to_string()
    }
}

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
}

pub trait ProcessRunner: Send + Sync {
    fn run(
        &self,
        spec: ProcessSpec,
        cancelled: Arc<AtomicBool>,
        on_output: &mut dyn FnMut(String),
    ) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
    fn run(
        &self,
        spec: ProcessSpec,
        cancelled: Arc<AtomicBool>,
        on_output: &mut dyn FnMut(String),
    ) -> Result<(), String> {
        if cancelled.load(Ordering::SeqCst) {
            return Err("ジョブはキャンセルされました".to_string());
        }

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(working_dir) = spec.working_dir {
            command.current_dir(working_dir);
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("外部プロセスの起動に失敗しました: {}", e))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (sender, receiver) = std::sync::mpsc::channel::<String>();

        if let Some(stdout) = stdout {
            let sender = sender.clone();
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = sender.send(line);
                }
            });
        }

        if let Some(stderr) = stderr {
            let sender = sender.clone();
            thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let _ = sender.send(line);
                }
            });
        }

        loop {
            while let Ok(line) = receiver.try_recv() {
                on_output(line);
            }

            if cancelled.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("ジョブはキャンセルされました".to_string());
            }

            match child
                .try_wait()
                .map_err(|e| format!("外部プロセスの終了確認に失敗しました: {}", e))?
            {
                Some(status) => {
                    while let Ok(line) = receiver.try_recv() {
                        on_output(line);
                    }

                    if status.success() {
                        return Ok(());
                    }

                    return Err(format!("外部プロセスが失敗しました: {}", status));
                }
                None => thread::sleep(std::time::Duration::from_millis(100)),
            }
        }
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("media")
        .to_string()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FakeRunner {
        pub programs: Mutex<Vec<String>>,
    }

    impl ProcessRunner for FakeRunner {
        fn run(
            &self,
            spec: ProcessSpec,
            _cancelled: Arc<AtomicBool>,
            on_output: &mut dyn FnMut(String),
        ) -> Result<(), String> {
            self.programs.lock().unwrap().push(spec.program);
            on_output("fake output".to_string());
            Ok(())
        }
    }
}
