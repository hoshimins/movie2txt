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

  useEffect(() => {
    const unlisten = listen<string>("transcription-log", (event) => {
      addLog(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
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
      const result = await invoke<string>("start_transcription", { filePath: selectedFile });
      setSrtFilePath(result);
      addLog("処理が完了しました");
    } catch (error) {
      addLog(`エラーが発生しました: ${error}`);
    } finally {
      setIsProcessing(false);
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
