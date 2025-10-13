import { useState } from "react";

function App() {
  const [selectedFile, setSelectedFile] = useState<string>("");
  const [logs, setLogs] = useState<string[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);

  return (
    <div className="container">
      <h1>Movie2Text - 動画文字起こしツール</h1>

      <div className="file-selector">
        <button>動画ファイルを選択</button>
        {selectedFile && <p>選択中: {selectedFile}</p>}
      </div>

      <div className="controls">
        <button disabled={!selectedFile || isProcessing}>
          変換開始
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
        <button disabled={true}>
          SRTファイルを開く
        </button>
      </div>
    </div>
  );
}

export default App;
