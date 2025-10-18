import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
    <div className="container">
      <h1>Movie2Text - 動画文字起こしツール</h1>

      <div className="file-selector">
        <button onClick={handleSelectFile} disabled={isProcessing}>
          動画ファイルを選択
        </button>
        {selectedFile && <p className="selected-file">選択中: {selectedFile}</p>}
      </div>

      <div className="settings">
        <label htmlFor="maxLineWidth">
          1行あたりの最大文字数:
          <input
            id="maxLineWidth"
            type="number"
            min="0"
            value={maxLineWidth}
            onChange={handleMaxLineWidthChange}
            disabled={isProcessing}
            className="max-line-width-input"
          />
          <span className="hint">(0 = 制限なし)</span>
        </label>
      </div>

      <div className="controls">
        <button
          onClick={handleStartTranscription}
          disabled={!selectedFile || isProcessing}
        >
          {isProcessing ? "処理中..." : "変換開始"}
        </button>
      </div>

      <div className="logs">
        <h3>処理ログ:</h3>
        <textarea
          readOnly
          value={logs.join("\n")}
          placeholder="ログがここに表示されます..."
        />
      </div>

      <div className="output">
        <button
          onClick={handleEditSubtitles}
          disabled={!srtFilePath}
        >
          字幕を編集
        </button>
        <button
          onClick={handleOpenSrt}
          disabled={!srtFilePath}
        >
          SRTファイルを開く
        </button>
      </div>
    </div>
  );
}

export default App;
