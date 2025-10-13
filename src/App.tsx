import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [selectedFile, setSelectedFile] = useState<string>("");
  const [logs, setLogs] = useState<string[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [srtFilePath, setSrtFilePath] = useState<string>("");

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
      // TODO: Implement Tauri command
      addLog("処理中...");
      // const result = await invoke("start_transcription", { filePath: selectedFile });
      // setSrtFilePath(result);
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
      // TODO: Implement Tauri command to open file
      addLog(`SRTファイルを開きます: ${srtFilePath}`);
    } catch (error) {
      addLog(`エラー: ${error}`);
    }
  };

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
