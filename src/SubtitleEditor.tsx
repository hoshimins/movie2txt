import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { generateHighlightRequest, importHighlightCandidates, saveClipMarkers, saveSubtitles } from "./api";
import type { ClipMarker, SubtitleEntry } from "./types";

interface SubtitleEditorProps {
  projectId?: string;
  srtFilePath: string;
  videoFilePath: string;
  initialMarkers?: ClipMarker[];
  onClose: () => void;
  onSave: () => void;
}

function SubtitleEditor({ projectId, srtFilePath, videoFilePath, initialMarkers = [], onClose, onSave }: SubtitleEditorProps) {
  const [entries, setEntries] = useState<SubtitleEntry[]>([]);
  const [markers, setMarkers] = useState<ClipMarker[]>(initialMarkers);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [highlightBusy, setHighlightBusy] = useState(false);
  const [highlightMessage, setHighlightMessage] = useState("");
  const [pendingCandidateMarkers, setPendingCandidateMarkers] = useState<ClipMarker[]>([]);
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
    setMarkers(initialMarkers);
  }, [initialMarkers]);

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
      if (projectId) {
        await saveSubtitles(projectId, entries);
      } else {
        await invoke("save_srt_file", { filePath: srtFilePath, entries });
      }
      if (projectId) {
        await saveClipMarkers(projectId, markers);
      }
      onSave();
    } catch (err) {
      setError(`保存に失敗しました: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const handleSaveMarkers = async () => {
    if (!projectId) return;

    try {
      setSaving(true);
      setError("");
      await saveClipMarkers(projectId, markers);
      onSave();
    } catch (err) {
      setError(`切り抜き候補の保存に失敗しました: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const handleAddMarker = () => {
    const activeEntry = activeSubtitleIndex
      ? entries.find((entry) => entry.index === activeSubtitleIndex)
      : null;
    const startMs = activeEntry
      ? Math.round(parseTimeToSeconds(activeEntry.start_time) * 1000)
      : Math.max(0, Math.round((currentTime - 5) * 1000));
    const endMs = activeEntry
      ? Math.round(parseTimeToSeconds(activeEntry.end_time) * 1000)
      : Math.round((currentTime + 10) * 1000);
    const titleSource = activeEntry?.text.trim() || `候補 ${markers.length + 1}`;

    setMarkers((prev) => [
      ...prev,
      {
        id: crypto.randomUUID(),
        startMs,
        endMs: Math.max(endMs, startMs + 1_000),
        title: titleSource.slice(0, 36),
        memo: "",
        tags: [],
        sourceSubtitleIds: activeEntry ? [activeEntry.index] : [],
      },
    ]);
  };

  const updateMarker = (id: string, patch: Partial<ClipMarker>) => {
    setMarkers((prev) =>
      prev.map((marker) => (marker.id === id ? { ...marker, ...patch } : marker)),
    );
  };

  const deleteMarker = (id: string) => {
    setMarkers((prev) => prev.filter((marker) => marker.id !== id));
  };

  const handleGenerateHighlightRequest = async () => {
    if (!projectId) return;

    try {
      setHighlightBusy(true);
      setHighlightMessage("");
      setError("");
      const bundle = await generateHighlightRequest(projectId, { maxCandidates: 20 });
      setHighlightMessage(`依頼ファイルを生成しました: ${bundle.requestPath}`);
      onSave();
    } catch (err) {
      setError(`見どころ依頼ファイルの生成に失敗しました: ${err}`);
    } finally {
      setHighlightBusy(false);
    }
  };

  const handleImportHighlightCandidates = async () => {
    if (!projectId) return;

    const selected = await open({
      multiple: false,
      filters: [{ name: "Highlight candidates", extensions: ["json"] }],
    });
    if (typeof selected !== "string") return;

    try {
      setHighlightBusy(true);
      setHighlightMessage("");
      setError("");
      const imported = await importHighlightCandidates(projectId, selected);
      setPendingCandidateMarkers(imported);
      setHighlightMessage(`${imported.length}件の見どころ候補を読み込みました。確認してからマーカー化してください。`);
    } catch (err) {
      setError(`見どころ候補の読み込みに失敗しました: ${err}`);
    } finally {
      setHighlightBusy(false);
    }
  };

  const handleAcceptCandidateMarkers = () => {
    setMarkers((prev) => mergeMarkers(prev, pendingCandidateMarkers));
    setHighlightMessage(`${pendingCandidateMarkers.length}件の候補をマーカーに追加しました。保存するとプロジェクトに反映されます。`);
    setPendingCandidateMarkers([]);
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
  const activeSubtitle = entries.find(entry => entry.index === activeSubtitleIndex);
  const hasActiveSubtitleText = Boolean(activeSubtitle?.text.trim());

  return (
    <div className="subtitle-editor">
      <div className="editor-header">
        <div>
          <p className="eyebrow">Editor</p>
          <h2>字幕編集</h2>
        </div>
        <div className="editor-actions">
          <button className="primary" onClick={handleSave} disabled={saving}>
            {saving ? "保存中..." : "保存"}
          </button>
          <button onClick={onClose}>閉じる</button>
        </div>
      </div>

      <div className="editor-notices">
        {error && <div className="error-message panel error-panel">{error}</div>}
        {videoError && <div className="error-message panel error-panel">動画エラー: {videoError}</div>}
      </div>

      <div className="editor-content">
        <div className="video-preview">
          <div className="video-stage">
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
            {activeSubtitle && hasActiveSubtitleText && (
              <div className="video-subtitle-overlay">
                {activeSubtitle.text}
              </div>
            )}
          </div>
          <div className="video-controls">
            <button onClick={handlePlayPause} className="primary" disabled={!videoLoaded}>
              {isPlaying ? "⏸ 一時停止" : "▶ 再生"}
            </button>
            <span className="video-time">
              {Math.floor(currentTime / 60)}:{String(Math.floor(currentTime % 60)).padStart(2, '0')}
            </span>
            {videoLoaded && <span className="video-status">✓ 準備完了</span>}
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
                <span className="subtitle-index">#{entry.index}</span>
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

        <div className="clip-marker-panel">
          <div className="clip-marker-header">
            <div>
              <p className="eyebrow">Clip</p>
              <h3>切り抜き候補</h3>
            </div>
            <div className="clip-marker-actions">
              <button onClick={handleGenerateHighlightRequest} disabled={!projectId || highlightBusy}>
                依頼生成
              </button>
              <button onClick={handleImportHighlightCandidates} disabled={!projectId || highlightBusy}>
                候補読込
              </button>
              <button onClick={handleAddMarker}>候補追加</button>
              <button onClick={handleSaveMarkers} disabled={!projectId || saving}>
                候補保存
              </button>
            </div>
          </div>
          {highlightMessage && <p className="field-hint">{highlightMessage}</p>}
          {pendingCandidateMarkers.length > 0 && (
            <div className="candidate-review">
              <div className="candidate-review-header">
                <strong>{pendingCandidateMarkers.length}件の候補</strong>
                <div>
                  <button className="primary" onClick={handleAcceptCandidateMarkers}>マーカー化</button>
                  <button onClick={() => setPendingCandidateMarkers([])}>破棄</button>
                </div>
              </div>
              <div className="candidate-review-list">
                {pendingCandidateMarkers.slice(0, 5).map((marker) => (
                  <div key={marker.id} className="candidate-review-item">
                    <strong>{marker.title}</strong>
                    <span>{marker.startMs}ms - {marker.endMs}ms</span>
                  </div>
                ))}
              </div>
            </div>
          )}
          <div className="clip-marker-list">
            {markers.map((marker) => (
              <article key={marker.id} className="clip-marker-item">
                <div className="marker-time-row">
                  <input
                    type="number"
                    value={marker.startMs}
                    onChange={(event) => updateMarker(marker.id, { startMs: Number(event.target.value) })}
                    title="開始ミリ秒"
                  />
                  <input
                    type="number"
                    value={marker.endMs}
                    onChange={(event) => updateMarker(marker.id, { endMs: Number(event.target.value) })}
                    title="終了ミリ秒"
                  />
                </div>
                <input
                  type="text"
                  value={marker.title}
                  onChange={(event) => updateMarker(marker.id, { title: event.target.value })}
                  placeholder="候補タイトル"
                />
                <textarea
                  value={marker.memo}
                  onChange={(event) => updateMarker(marker.id, { memo: event.target.value })}
                  placeholder="編集メモ"
                  rows={2}
                />
                <div className="marker-footer">
                  <span>字幕: {marker.sourceSubtitleIds.join(", ") || "未紐付け"}</span>
                  <button className="delete-btn" onClick={() => deleteMarker(marker.id)}>削除</button>
                </div>
              </article>
            ))}
            {markers.length === 0 && <p className="field-hint">再生位置または選択中の字幕から候補を追加できます。</p>}
          </div>
        </div>
      </div>
    </div>
  );
}

export default SubtitleEditor;

function mergeMarkers(current: ClipMarker[], imported: ClipMarker[]) {
  const existingIds = new Set(current.map((marker) => marker.id));
  return [
    ...current,
    ...imported.filter((marker) => !existingIds.has(marker.id)),
  ];
}
