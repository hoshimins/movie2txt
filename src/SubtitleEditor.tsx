import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";

interface SubtitleEntry {
  index: number;
  start_time: string;
  end_time: string;
  text: string;
}

interface SubtitleEditorProps {
  srtFilePath: string;
  videoFilePath: string;
  onClose: () => void;
  onSave: () => void;
}

function SubtitleEditor({ srtFilePath, videoFilePath, onClose, onSave }: SubtitleEditorProps) {
  const [entries, setEntries] = useState<SubtitleEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>("");
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [activeSubtitleIndex, setActiveSubtitleIndex] = useState<number | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    loadSubtitles();
  }, [srtFilePath]);

  const loadSubtitles = async () => {
    try {
      setLoading(true);
      setError("");
      const data = await invoke<SubtitleEntry[]>("read_srt_file", { filePath: srtFilePath });
      setEntries(data);
    } catch (err) {
      setError(`字幕の読み込みに失敗しました: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleTextChange = (index: number, newText: string) => {
    setEntries(prev => prev.map(entry =>
      entry.index === index ? { ...entry, text: newText } : entry
    ));
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      setError("");
      await invoke("save_srt_file", { filePath: srtFilePath, entries });
      onSave();
    } catch (err) {
      setError(`保存に失敗しました: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const parseTimeToSeconds = (time: string): number => {
    const parts = time.split(':');
    const hours = parseInt(parts[0]);
    const minutes = parseInt(parts[1]);
    const secondsParts = parts[2].split(',');
    const seconds = parseInt(secondsParts[0]);
    const milliseconds = parseInt(secondsParts[1]);
    return hours * 3600 + minutes * 60 + seconds + milliseconds / 1000;
  };

  const handleTimeUpdate = () => {
    if (videoRef.current) {
      const time = videoRef.current.currentTime;
      setCurrentTime(time);

      const activeIndex = entries.findIndex((entry) => {
        const start = parseTimeToSeconds(entry.start_time);
        const end = parseTimeToSeconds(entry.end_time);
        return time >= start && time <= end;
      });

      setActiveSubtitleIndex(activeIndex >= 0 ? entries[activeIndex].index : null);
    }
  };

  const handlePlayPause = () => {
    if (videoRef.current) {
      if (isPlaying) {
        videoRef.current.pause();
      } else {
        videoRef.current.play();
      }
      setIsPlaying(!isPlaying);
    }
  };

  const handleSeekToSubtitle = (index: number) => {
    if (videoRef.current) {
      const entry = entries.find(e => e.index === index);
      if (entry) {
        const time = parseTimeToSeconds(entry.start_time);
        videoRef.current.currentTime = time;
      }
    }
  };

  if (loading) {
    return <div className="subtitle-editor loading">読み込み中...</div>;
  }

  if (error && entries.length === 0) {
    return (
      <div className="subtitle-editor error">
        <p>{error}</p>
        <button onClick={onClose}>閉じる</button>
      </div>
    );
  }

  const videoSrc = convertFileSrc(videoFilePath);

  return (
    <div className="subtitle-editor">
      <div className="editor-header">
        <h2>字幕編集</h2>
        <div className="editor-actions">
          <button onClick={handleSave} disabled={saving}>
            {saving ? "保存中..." : "保存"}
          </button>
          <button onClick={onClose}>閉じる</button>
        </div>
      </div>

      {error && <div className="error-message">{error}</div>}

      <div className="editor-content">
        <div className="video-preview">
          <video
            ref={videoRef}
            src={videoSrc}
            onTimeUpdate={handleTimeUpdate}
            onPlay={() => setIsPlaying(true)}
            onPause={() => setIsPlaying(false)}
            className="video-player"
          />
          <div className="video-controls">
            <button onClick={handlePlayPause} className="control-btn">
              {isPlaying ? "⏸ 一時停止" : "▶ 再生"}
            </button>
            <span className="video-time">
              {Math.floor(currentTime / 60)}:{String(Math.floor(currentTime % 60)).padStart(2, '0')}
            </span>
          </div>
        </div>

        <div className="subtitle-list">
          {entries.map((entry) => (
            <div
              key={entry.index}
              className={`subtitle-item ${activeSubtitleIndex === entry.index ? 'active' : ''}`}
              onClick={() => handleSeekToSubtitle(entry.index)}
            >
              <div className="subtitle-header">
                <span className="subtitle-index">#{entry.index}</span>
                <span className="subtitle-time">
                  {entry.start_time} → {entry.end_time}
                </span>
              </div>
              <textarea
                value={entry.text}
                onChange={(e) => handleTextChange(entry.index, e.target.value)}
                className="subtitle-text"
                rows={3}
                onClick={(e) => e.stopPropagation()}
              />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

export default SubtitleEditor;
