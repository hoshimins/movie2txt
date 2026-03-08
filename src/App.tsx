import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import SubtitleEditor from "./SubtitleEditor";

function App() {
  const [selectedFile, setSelectedFile] = useState<string>("");
  const [logs, setLogs] = useState<string[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [srtFilePath, setSrtFilePath] = useState<string>("");
  const [showEditor, setShowEditor] = useState(false);
  const [maxLineWidth, setMaxLineWidth] = useState<number>(23);

  useEffect(() => {
    const unlisten = listen<string>("transcription-log", (event) => {
      addLog(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    // localStorageから設定を読み込む
    const saved = localStorage.getItem("maxLineWidth");
    if (saved) {
      const value = parseInt(saved);
      if (!isNaN(value) && value >= 0) {
        setMaxLineWidth(value);
      }
    }
  }, []);

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Video',
          extensions: ['mp4', 'avi', 'mov', 'mkv', 'flv', 'wmv']
        }]
      });

      if (selected && typeof selected === 'string') {
        setSelectedFile(selected);
        addLog(`ファイルを選択しました: ${selected}`);
      }
    } catch (error) {
      addLog(`エラー: ${error}`);
    }
  };

  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev, `[${timestamp}] ${message}`]);
  };

  const handleStartTranscription = async () => {
    if (!selectedFile) return;

    setIsProcessing(true);
    setSrtFilePath("");
    addLog("文字起こしを開始します...");

    try {
      const maxLineWidthParam = maxLineWidth > 0 ? maxLineWidth : null;
      const result = await invoke<string>("start_transcription", {
        filePath: selectedFile,
        maxLineWidth: maxLineWidthParam
      });
      setSrtFilePath(result);
      addLog("処理が完了しました");
    } catch (error) {
      addLog(`エラーが発生しました: ${error}`);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleMaxLineWidthChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    if (value === "") {
      setMaxLineWidth(0);
      return;
    }

    const numValue = parseInt(value);
    if (!isNaN(numValue) && numValue >= 0) {
      setMaxLineWidth(numValue);
      // localStorageに保存
      localStorage.setItem("maxLineWidth", numValue.toString());
    }
  };

  const handleOpenSrt = async () => {
    if (!srtFilePath) return;

    try {
      await invoke("open_srt_file", { filePath: srtFilePath });
      addLog(`SRTファイルを開きます: ${srtFilePath}`);
    } catch (error) {
      addLog(`エラー: ${error}`);
    }
  };

  const handleEditSubtitles = () => {
    if (!srtFilePath) return;
    setShowEditor(true);
  };

  const handleEditorClose = () => {
    setShowEditor(false);
  };

  const handleEditorSave = () => {
    addLog("字幕を保存しました");
    setShowEditor(false);
  };

  const selectedFileName = selectedFile ? selectedFile.split(/[\\/]/).pop() ?? selectedFile : "";
  const outputFileName = srtFilePath ? srtFilePath.split(/[\\/]/).pop() ?? srtFilePath : "";
  const statusLabel = isProcessing ? "処理中" : srtFilePath ? "完了" : selectedFile ? "待機中" : "未選択";
  const logCount = logs.length;
  const latestLog = logs.length > 0 ? logs[logs.length - 1] : "システムログがここに表示されます。";

  if (showEditor && srtFilePath) {
    return (
      <SubtitleEditor
        srtFilePath={srtFilePath}
        videoFilePath={selectedFile}
        onClose={handleEditorClose}
        onSave={handleEditorSave}
      />
    );
  }

  return (
    <div className="app-layout">
      <header className="app-header">
        <div>
          <h1>Movie2Text</h1>
        </div>
        <div className={`status-pill ${isProcessing ? "is-busy" : srtFilePath ? "is-ready" : ""}`}>
          <span className="status-dot" />
          {statusLabel}
        </div>
      </header>

      <div className="sidebar">
        <section className="panel hero-panel">
          <h2>動画ファイル</h2>
          <button className="primary hero-button" onClick={handleSelectFile} disabled={isProcessing}>
            動画を選択
          </button>
          <div className="selected-file-card" title={selectedFile || "ファイルが選択されていません"}>
            <span className="selected-file-label">選択中</span>
            <strong>{selectedFileName || "ファイルが選択されていません"}</strong>
            <span className="selected-file-path">{selectedFile || "mp4 / mov / mkv / avi / flv / wmv"}</span>
          </div>
        </section>

        <section className="panel settings-panel">
          <div className="panel-heading">
            <div>
              <h3>設定</h3>
            </div>
            <span className="metric-chip">{maxLineWidth === 0 ? "無制限" : `${maxLineWidth} 文字`}</span>
          </div>
          <label className="field-label" htmlFor="maxLineWidth">
            1行あたりの最大文字数
          </label>
          <input
            id="maxLineWidth"
            type="number"
            min="0"
            value={maxLineWidth}
            onChange={handleMaxLineWidthChange}
            disabled={isProcessing}
            className="w-full"
          />
          <p className="field-hint">`0` を指定すると自動分割を無効化します。設定はこの PC の `localStorage` に保存されます。</p>
          <button
            className="primary w-full"
            onClick={handleStartTranscription}
            disabled={!selectedFile || isProcessing}
          >
            {isProcessing ? "文字起こし中..." : "字幕を生成"}
          </button>
        </section>

        <section className="stats-grid">
          <article className="panel stat-card">
            <span className="stat-label">ログ</span>
            <strong>{logCount}</strong>
            <span className="stat-meta">件</span>
          </article>
          <article className="panel stat-card">
            <span className="stat-label">出力</span>
            <strong>{outputFileName || "未生成"}</strong>
            <span className="stat-meta">SRT</span>
          </article>
        </section>

        <section className="action-group">
          <button
            className="secondary-action"
            onClick={handleEditSubtitles}
            disabled={!srtFilePath}
          >
            字幕編集
          </button>
          <button
            className="secondary-action"
            onClick={handleOpenSrt}
            disabled={!srtFilePath}
          >
            フォルダを開く
          </button>
        </section>
      </div>

      <main className="main-content">
        <section className="panel overview-panel">
          <div className="overview-copy">
            <h2>状態</h2>
          </div>
          <div className="overview-cards">
            <article className="overview-card">
              <span>動画</span>
              <strong>{selectedFileName || "未選択"}</strong>
            </article>
            <article className="overview-card">
              <span>文字数制限</span>
              <strong>{maxLineWidth === 0 ? "無制限" : `${maxLineWidth} 文字`}</strong>
            </article>
            <article className="overview-card">
              <span>出力</span>
              <strong>{outputFileName || "未生成"}</strong>
            </article>
          </div>
        </section>

        <section className="panel logs-container">
          <div className="logs-toolbar">
            <div>
              <div className="logs-header">System Logs</div>
              <p className="logs-subtitle">{latestLog}</p>
            </div>
            <button onClick={() => setLogs([])} disabled={isProcessing || logs.length === 0}>
              ログをクリア
            </button>
          </div>
          <textarea
            className="logs-display"
            readOnly
            value={logs.join("\n")}
            placeholder="システムログがここに表示されます..."
          />
        </section>
      </main>
    </div>
  );
}

export default App;
