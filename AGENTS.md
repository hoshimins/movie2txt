このファイルは、このリポジトリでコードを扱う際のClaude Code (claude.ai/code) へのガイダンスです。

## プロジェクト概要

**Movie2Text**は、Tauri 2.0 + React + TypeScriptで構築されたWindowsデスクトップアプリケーションです。Whisper AI (faster-whisper.exe) を使用して動画ファイルを自動的にSRT字幕ファイルに書き起こします。

主な機能：
- 動画ファイルの選択とffmpegによる音声の前処理
- Whisper large-v3モデル（CUDA高速化）を使用した日本語音声認識
- 処理中のリアルタイムログ表示
- 動画プレビューとタイムラインを備えたインタラクティブな字幕エディタ
- 文字数制限に基づく自動的な時間分割処理

## 開発コマンド

### セットアップ
```bash
npm install                    # 依存関係のインストール

```

### 開発

```bash
npm run dev                    # Vite開発サーバーのみ起動（フロントエンドのみ）
npm run tauri dev              # Tauri開発モード起動（アプリ全体の開発に推奨）

```

### ビルド

```bash
npm run build                  # フロントエンドのみビルド（TSコンパイル + Viteビルド）
npm run tauri build            # アプリケーション全体のビルド（src-tauri/target/release/ にインストーラー作成）

```

### その他のTauriコマンド

```bash
npm run tauri <command>        # 任意のTauri CLIコマンドを実行

```

## 環境設定 (.env)

プロジェクトルートに以下の変数を含む `.env` ファイルが必要です：

```env
FFMPEG_PATH=<ffmpeg.exeへの絶対パス>
WHISPER_PATH=<faster-whisper.exeへの絶対パス>
TMP_DIR=<一時ディレクトリへの絶対パス>
OUTPUT_DIR=<出力ディレクトリへの絶対パス>

```

これらのパスはRustバックエンドで `dotenvy` クレートを使用して読み込まれます。これらが正しく設定されていない場合、書き起こし処理は開始されません。

## アーキテクチャ

### フロントエンド (React + TypeScript)

* **src/main.tsx**: エントリーポイント
* **src/App.tsx**: メインUI
* Tauriダイアログプラグインによるファイル選択
* 書き起こしプロセスの開始
* Rustバックエンドからのイベントログ表示
* 設定（最大行幅）のlocalStorageへの保存


* **src/SubtitleEditor.tsx**: 字幕エディタ
* タイムライン可視化付き動画プレビュー
* 字幕編集（テキスト、開始/終了時間）
* カーソル位置での分割、削除、ドラッグによる時間調整


* **src/styles.css**: グローバルスタイル（ダークテーマ）

### バックエンド (Rust + Tauri)

* **src-tauri/src/main.rs**: エントリーポイント
* **src-tauri/src/lib.rs**: 初期化とコマンド登録
* **src-tauri/src/commands.rs**: コアビジネスロジック
* `start_transcription`: ffmpeg → Whisper パイプラインの制御
* `open_srt_file`: エクスプローラーでSRTを開く
* `read_srt_file`/`save_srt_file`: SRTのパースと保存
* 文字数制限ロジック: 長い字幕の分割と時間の比例配分



### 通信フロー

1. **Frontend → Backend**: `@tauri-apps/api/core` の `invoke()` でRustコマンドを呼び出し
2. **Backend → Frontend**: Tauriのイベントシステム (`app.emit()`) でログを送信
3. **動画読み込み**: `convertFileSrc()` を使用してアセットプロトコルURLに変換

## 実装の重要ポイント

### 書き起こしパイプライン (`start_transcription`)

1. **環境セットアップ**: .env読み込み、ディレクトリ作成
2. **FFmpeg処理**: 動画を16kHzモノラルWAVに変換
* Windows特有の `creation_flags(0x08000000)` でコンソールウィンドウを隠蔽


3. **Whisper処理**: WAVをSRTに変換
* モデル: large-v3, 言語: ja, デバイス: CUDA (float16)
* VADフィルター有効


4. **文字数制限（オプション）**: `max_line_width` が設定されている場合、SRTをパースして再分割

### プラットフォーム固有の注記

* **Windows専用**: Windows 11向けに設計
* **CUDA必須**: Whisper処理にはNVIDIA GPUが必要
* **コンソール隠蔽**: 外部プロセス（ffmpeg/whisper）起動時にウィンドウが出ないよう制御

---

# トラブルシューティングとデバッグガイド

Tauri/Rust/React環境での一般的な問題解決ガイドです。

## デバッグの優先順位

### 1. シンプルな解決策を最初に試す

* 開発サーバーの再起動 (`npm run tauri dev`)
* **Rustのクリーン**: `cd src-tauri && cargo clean` (ビルドがおかしい場合)
* フロントエンドの再インストール: `rm -rf node_modules && npm install`

### 2. ログの確認場所を使い分ける

* **フロントエンドの挙動**: ブラウザの開発者ツール（右クリック → 検証、または `Ctrl+Shift+I`）のConsoleタブ
* **バックエンドの挙動**: コマンドライン/ターミナルの出力。Rust側の `println!` や `eprintln!` はここに表示されます。
* **外部プロセスエラー**: ffmpegやWhisperのエラーは、Rust側でキャッチしてフロントエンドにイベントとして送られるか、ターミナルに出力されます。

## ビルド・実行時の問題と解決方法

### 症状: "Transcription fails immediately" (書き起こしが即座に失敗する)

**原因**: 環境変数のパス設定ミス、または外部実行ファイル（ffmpeg/whisper）が見つからない/実行権限がない。

**解決方法**:

1. `.env` ファイルのパスが絶対パスで、かつ`\`がエスケープされているか、あるいは `/` を使用しているか確認してください。
2. 指定したパスに `.exe` ファイルが実際に存在するか確認してください。
3. Rust側のログを確認し、どの段階（FFmpegかWhisperか）で落ちているか特定してください。

### 症状: "Core dumped" や CUDA関連のエラー

**原因**: NVIDIAドライバーが古い、またはCUDA Toolkitがインストールされていない。

**解決方法**:

* NVIDIAドライバーを最新に更新してください。
* `nvidia-smi` コマンドが通るか確認してください。
* `faster-whisper.exe` が依存しているcuDNNなどのDLLがパスに通っているか確認してください。

### 症状: コード変更が反映されない

**原因**: Tauriのビルドキャッシュ、またはViteのHMR（ホットリロード）の問題。

**解決方法**:

#### フロントエンドの変更が反映されない場合

```bash
# Viteキャッシュ削除
rm -rf node_modules/.vite
npm run tauri dev

```

#### バックエンド (Rust) の変更が反映されない場合

Rustはコンパイル言語なので、変更後は再コンパイルが必要です。`npm run tauri dev` は通常自動で再ビルドしますが、挙動がおかしい場合は：

```bash
# src-tauri ディレクトリで
cd src-tauri
cargo clean
cd ..
npm run tauri dev

```

### 症状: コンソールウィンドウが一瞬表示される

**原因**: `Command::new` でプロセスを起動する際、Windows固有のフラグが設定されていない。

**確認**:
`src-tauri/src/commands.rs` で以下のような記述があるか確認してください：

```rust
use std::os::windows::process::CommandExt;
// ...
.creation_flags(0x08000000) // NO_WINDOW flag

```

## よくあるエラーと対処

### 問題: `invoke` でRustコマンドが見つからないと言われる

**症状**: フロントエンドのコンソールに `command not found` エラーが出る。

**解決**:

1. `src-tauri/src/commands.rs` で関数に `#[tauri::command]` がついているか。
2. `src-tauri/src/lib.rs` の `tauri::Builder::default().invoke_handler(...)` に関数名が登録されているか。
3. フロントエンドの `invoke("command_name")` の名前がRust関数名と一致しているか（スネークケース）。

### 問題: 日本語のファイルパスでエラーになる

**原因**: Windowsのパスエンコーディング問題、またはFFmpeg/Whisperへの引数渡し時の文字化け。

**解決**:
Rust側で `PathBuf` を扱う際、`.to_str()` や `.to_string_lossy()` で適切に変換しているか確認してください。本アプリではTauriのアセットプロトコルを使用しているため、URLエンコード周りにも注意が必要です。

## デバッグのベストプラクティス

### Rust側のデバッグ出力

```rust
// 単純な出力
println!("Variable: {:?}", variable);

// 標準エラー出力（ログに埋もれにくい）
eprintln!("Error happened: {:?}", error);

// フロントエンドへイベント送信（UIで確認したい場合）
app.emit("log-message", format!("Debug: {:?}", variable)).unwrap();

```

### フロントエンド側のデバッグ

```typescript
// Rustからの戻り値を確認
try {
  const result = await invoke('command_name', { ... });
  console.log('Result:', result);
} catch (e) {
  console.error('Rust error:', e);
}

```