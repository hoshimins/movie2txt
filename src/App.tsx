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
      {/* Header */}
      <header className="app-header">
        <h1>Movie2Text AI</h1>
      </header>

      {/* Sidebar - Settings & File Selection */}
      <div className="sidebar">

        {/* File Selector */}
        <section className="panel file-drop-area">
          <button className="primary" onClick={handleSelectFile} disabled={isProcessing}>
            動画を選択
          </button>

          {selectedFile ? (
            <div className="selected-file-badge" title={selectedFile}>
              {selectedFile.split(/[\\/]/).pop()}
            </div>
          ) : (
            <p className="text-muted text-sm">ファイルが選択されていません</p>
          )}
        </section>

        {/* Global Controls */}
        <section className="panel flex flex-col gap-4">
          <label className="text-sm font-bold text-muted" htmlFor="maxLineWidth">
            文字数制限 (0=無制限)
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

          <button
            className="primary w-full"
            onClick={handleStartTranscription}
            disabled={!selectedFile || isProcessing}
          >
            {isProcessing ? "文字起こし中..." : "開始"}
          </button>
        </section>

        {/* Action Buttons */}
        <section className="flex flex-col gap-2">
          <button
            onClick={handleEditSubtitles}
            disabled={!srtFilePath}
          >
            字幕編集
          </button>
          <button
            onClick={handleOpenSrt}
            disabled={!srtFilePath}
          >
            フォルダを開く
          </button>
        </section>
      </div>

      {/* Main Content Area */}
      <main className="main-content">

        {showEditor && srtFilePath ? (
          <SubtitleEditor
            srtFilePath={srtFilePath}
            videoFilePath={selectedFile}
            onClose={handleEditorClose}
            onSave={handleEditorSave}
          />
        ) : (
          <div className="panel logs-container">
            <div className="logs-header">
              System Logs
            </div>
            <textarea
              className="logs-display"
              readOnly
              value={logs.join("\n")}
              placeholder="システムログがここに表示されます..."
            />
          </div>
        )}

      </main>
    </div>
  );
}

export default App;
