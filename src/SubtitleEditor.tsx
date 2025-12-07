import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

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
  const [videoError, setVideoError] = useState<string>("");
  const [videoLoaded, setVideoLoaded] = useState(false);
  const [videoDuration, setVideoDuration] = useState(0);
  const [draggingEntry, setDraggingEntry] = useState<{ index: number; edge: 'start' | 'end' } | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const timelineRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadSubtitles();
  }, [srtFilePath]);

  useEffect(() => {
    console.log("Video file path:", videoFilePath);
    if (videoFilePath) {
      const convertedSrc = convertFileSrc(videoFilePath);
      console.log("Converted video src:", convertedSrc);
    }
  }, [videoFilePath]);

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

  const handleTimeChange = (index: number, field: 'start_time' | 'end_time', newTime: string) => {
    // Basic validation: check format HH:MM:SS,mmm
    const timeRegex = /^\d{2}:\d{2}:\d{2},\d{3}$/;
    if (!timeRegex.test(newTime)) {
      return; // Invalid format, don't update
    }

    setEntries(prev => prev.map(entry =>
      entry.index === index ? { ...entry, [field]: newTime } : entry
    ));
  };

  const handleSplitEntry = (index: number, cursorPosition: number) => {
    const entry = entries.find(e => e.index === index);
    if (!entry) return;

    const text = entry.text;
    const beforeText = text.substring(0, cursorPosition).trim();
    const afterText = text.substring(cursorPosition).trim();

    if (!beforeText || !afterText) {
      alert("分割位置の前後にテキストが必要です");
      return;
    }

    // 時間を計算して分割
    const startSec = parseTimeToSeconds(entry.start_time);
    const endSec = parseTimeToSeconds(entry.end_time);
    const duration = endSec - startSec;

    // 文字数の比率で時間を分割
    const totalChars = beforeText.length + afterText.length;
    const ratio = totalChars > 0 ? beforeText.length / totalChars : 0.5;
    const midSec = startSec + (duration * ratio);

    const formatTime = (seconds: number): string => {
      const hrs = Math.floor(seconds / 3600);
      const mins = Math.floor((seconds % 3600) / 60);
      const secs = Math.floor(seconds % 60);
      const millis = Math.round((seconds % 1) * 1000);
      return `${String(hrs).padStart(2, '0')}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')},${String(millis).padStart(3, '0')}`;
    };

    const midTime = formatTime(midSec);

    setEntries(prev => {
      const entryIdx = prev.findIndex(e => e.index === index);
      if (entryIdx === -1) return prev;

      const newEntries = [...prev];

      // 既存エントリを更新
      newEntries[entryIdx] = {
        ...entry,
        end_time: midTime,
        text: beforeText,
      };

      // 新しいエントリを挿入
      newEntries.splice(entryIdx + 1, 0, {
        index: index + 1,
        start_time: midTime,
        end_time: entry.end_time,
        text: afterText,
      });

      // 後続のエントリのインデックスを更新
      for (let i = entryIdx + 2; i < newEntries.length; i++) {
        newEntries[i] = { ...newEntries[i], index: newEntries[i].index + 1 };
      }

      return newEntries;
    });
  };

  const handleDeleteEntry = (index: number) => {
    if (!confirm("この字幕エントリを削除しますか？")) {
      return;
    }

    setEntries(prev => {
      const entryIdx = prev.findIndex(e => e.index === index);
      if (entryIdx === -1) return prev;

      const newEntries = [...prev];
      // エントリを削除
      newEntries.splice(entryIdx, 1);

      // 後続のエントリのインデックスを再採番
      for (let i = entryIdx; i < newEntries.length; i++) {
        newEntries[i] = { ...newEntries[i], index: i + 1 };
      }

      return newEntries;
    });
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

  const handleVideoError = (e: React.SyntheticEvent<HTMLVideoElement, Event>) => {
    const videoElement = e.currentTarget;
    let errorMessage = "動画の読み込みに失敗しました";

    if (videoElement.error) {
      switch (videoElement.error.code) {
        case MediaError.MEDIA_ERR_ABORTED:
          errorMessage = "動画の読み込みが中断されました";
          break;
        case MediaError.MEDIA_ERR_NETWORK:
          errorMessage = "ネットワークエラーで動画を読み込めませんでした";
          break;
        case MediaError.MEDIA_ERR_DECODE:
          errorMessage = "動画のデコードに失敗しました";
          break;
        case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
          errorMessage = "この動画形式はサポートされていません";
          break;
      }
      console.error("Video error:", videoElement.error);
    }
    setVideoError(errorMessage);
  };

  const handleVideoLoadedMetadata = () => {
    console.log("Video metadata loaded successfully");
    if (videoRef.current) {
      setVideoDuration(videoRef.current.duration);
    }
    setVideoLoaded(true);
    setVideoError("");
  };

  const handleVideoCanPlay = () => {
    console.log("Video can play");
  };

  const formatTime = (seconds: number): string => {
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    const millis = Math.round((seconds % 1) * 1000);
    return `${String(hrs).padStart(2, '0')}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')},${String(millis).padStart(3, '0')}`;
  };

  const handleTimelineClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!timelineRef.current || !videoRef.current || videoDuration === 0) return;

    const rect = timelineRef.current.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = x / rect.width;
    const time = percentage * videoDuration;

    videoRef.current.currentTime = Math.max(0, Math.min(time, videoDuration));
  };

  const handleTimelineDragStart = (index: number, edge: 'start' | 'end', e: React.MouseEvent) => {
    e.stopPropagation();
    setDraggingEntry({ index, edge });
  };

  const handleTimelineDrag = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!draggingEntry || !timelineRef.current || videoDuration === 0) return;

    const rect = timelineRef.current.getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
    const percentage = x / rect.width;
    const newTime = percentage * videoDuration;

    const entry = entries.find(e => e.index === draggingEntry.index);
    if (!entry) return;

    const startSec = parseTimeToSeconds(entry.start_time);
    const endSec = parseTimeToSeconds(entry.end_time);

    setEntries(prev => prev.map(e => {
      if (e.index !== draggingEntry.index) return e;

      if (draggingEntry.edge === 'start') {
        const newStartTime = Math.max(0, Math.min(newTime, endSec - 0.1));
        return { ...e, start_time: formatTime(newStartTime) };
      } else {
        const newEndTime = Math.max(startSec + 0.1, Math.min(newTime, videoDuration));
        return { ...e, end_time: formatTime(newEndTime) };
      }
    }));
  };

  const handleTimelineDragEnd = () => {
    setDraggingEntry(null);
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

  if (!videoFilePath) {
    return (
      <div className="subtitle-editor error">
        <p>動画ファイルのパスが見つかりません</p>
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
          <button className="primary" onClick={handleSave} disabled={saving}>
            {saving ? "保存中..." : "保存"}
          </button>
          <button onClick={onClose}>閉じる</button>
        </div>
      </div>

      {error && <div className="error-message panel" style={{ borderColor: 'var(--error)' }}>{error}</div>}
      {videoError && <div className="error-message panel" style={{ borderColor: 'var(--error)' }}>動画エラー: {videoError}</div>}

      <div className="editor-content">
        <div className="video-preview">
          {!videoLoaded && !videoError && (
            <div className="video-loading">動画を読み込んでいます...</div>
          )}
          <video
            ref={videoRef}
            src={videoSrc}
            onTimeUpdate={handleTimeUpdate}
            onPlay={() => setIsPlaying(true)}
            onPause={() => setIsPlaying(false)}
            onError={handleVideoError}
            onLoadedMetadata={handleVideoLoadedMetadata}
            onCanPlay={handleVideoCanPlay}
            controls
            className="video-player"
          />
          <div className="video-controls">
            <button onClick={handlePlayPause} className="primary" disabled={!videoLoaded}>
              {isPlaying ? "⏸ 一時停止" : "▶ 再生"}
            </button>
            <span className="video-time">
              {Math.floor(currentTime / 60)}:{String(Math.floor(currentTime % 60)).padStart(2, '0')}
            </span>
            {videoLoaded && <span className="video-status" style={{ color: 'var(--success)' }}>✓ 準備完了</span>}
          </div>

          {/* Timeline */}
          {videoLoaded && videoDuration > 0 && (
            <div className="timeline-container">
              <div className="timeline-header">
                <span>タイムライン</span>
                <span className="timeline-duration">
                  {Math.floor(videoDuration / 60)}:{String(Math.floor(videoDuration % 60)).padStart(2, '0')}
                </span>
              </div>
              <div
                ref={timelineRef}
                className="timeline"
                onClick={handleTimelineClick}
                onMouseMove={handleTimelineDrag}
                onMouseUp={handleTimelineDragEnd}
                onMouseLeave={handleTimelineDragEnd}
              >
                {/* Current time indicator */}
                <div
                  className="timeline-cursor"
                  style={{ left: `${(currentTime / videoDuration) * 100}%` }}
                />

                {/* Subtitle entries on timeline */}
                {entries.map((entry) => {
                  const startSec = parseTimeToSeconds(entry.start_time);
                  const endSec = parseTimeToSeconds(entry.end_time);
                  const left = (startSec / videoDuration) * 100;
                  const width = ((endSec - startSec) / videoDuration) * 100;

                  return (
                    <div
                      key={entry.index}
                      className={`timeline-entry ${activeSubtitleIndex === entry.index ? 'active' : ''}`}
                      style={{
                        left: `${left}%`,
                        width: `${width}%`,
                      }}
                      title={`#${entry.index}: ${entry.start_time} → ${entry.end_time}`}
                    >
                      <div
                        className="timeline-entry-edge timeline-entry-start"
                        onMouseDown={(e) => handleTimelineDragStart(entry.index, 'start', e)}
                      />
                      <div className="timeline-entry-label">#{entry.index}</div>
                      <div
                        className="timeline-entry-edge timeline-entry-end"
                        onMouseDown={(e) => handleTimelineDragStart(entry.index, 'end', e)}
                      />
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        <div className="subtitle-list">
          {entries.map((entry) => (
            <div
              key={entry.index}
              className={`subtitle-item ${activeSubtitleIndex === entry.index ? 'active' : ''}`}
              onClick={() => handleSeekToSubtitle(entry.index)}
            >
              <div className="subtitle-header">
                <span className="subtitle-index" style={{ color: 'var(--accent-primary)' }}>#{entry.index}</span>
                <div className="subtitle-time-inputs">
                  <input
                    type="text"
                    className="time-input"
                    value={entry.start_time}
                    onChange={(e) => handleTimeChange(entry.index, 'start_time', e.target.value)}
                    onClick={(e) => e.stopPropagation()}
                    placeholder="00:00:00,000"
                    title="開始時間"
                  />
                  <span className="time-arrow">→</span>
                  <input
                    type="text"
                    className="time-input"
                    value={entry.end_time}
                    onChange={(e) => handleTimeChange(entry.index, 'end_time', e.target.value)}
                    onClick={(e) => e.stopPropagation()}
                    placeholder="00:00:00,000"
                    title="終了時間"
                  />
                </div>
                <div className="subtitle-actions">
                  <button
                    className="split-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      const textarea = document.querySelector(`textarea[data-index="${entry.index}"]`) as HTMLTextAreaElement;
                      const cursorPos = textarea?.selectionStart || Math.floor(entry.text.length / 2);
                      handleSplitEntry(entry.index, cursorPos);
                    }}
                    title="カーソル位置で分割"
                  >
                    ✂
                  </button>
                  <button
                    className="delete-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteEntry(entry.index);
                    }}
                    title="この字幕を削除"
                  >
                    🗑
                  </button>
                </div>
              </div>
              <textarea
                data-index={entry.index}
                value={entry.text}
                onChange={(e) => handleTextChange(entry.index, e.target.value)}
                className="subtitle-text"
                rows={2}
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
