import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SubtitleEntry {
  index: number;
  start_time: string;
  end_time: string;
  text: string;
}

interface SubtitleEditorProps {
  srtFilePath: string;
  onClose: () => void;
  onSave: () => void;
}

function SubtitleEditor({ srtFilePath, onClose, onSave }: SubtitleEditorProps) {
  const [entries, setEntries] = useState<SubtitleEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>("");

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

      <div className="subtitle-list">
        {entries.map((entry) => (
          <div key={entry.index} className="subtitle-item">
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
            />
          </div>
        ))}
      </div>
    </div>
  );
}

export default SubtitleEditor;
