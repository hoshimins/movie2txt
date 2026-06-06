# Movie2Text

URLまたはローカル動画から切り抜き準備用の素材・字幕・候補マーカーを作成するWindows用デスクトップアプリケーション

## 概要

YouTube等のURLから `yt-dlp` で素材を取得し、`ffmpeg` と `faster-whisper.exe` でSRT字幕を生成します。既存のローカル動画もプロジェクト化でき、字幕編集と切り抜き候補マーカーの保存まで一つの導線で扱うTauri + React製デスクトップアプリです。

## 主な機能

- URLからの動画/音声ダウンロード（yt-dlp）
- 動画ファイルの選択（mp4, avi, mov, mkv, flv, wmv）
- ffmpegによる音声前処理（16kHz mono変換）
- Whisper (large-v3)による高精度な日本語文字起こし
- Premiere向けに1字幕あたりの文字数を抑えた短いSRTエントリへの再分割
- プロジェクト単位の再開
- リアルタイムジョブログ表示
- 字幕エディタと切り抜き候補マーカー管理

## 必要な環境

- Windows 11
- CUDA対応GPU（Whisper処理用）
- ffmpeg.exe
- faster-whisper.exe

## セットアップ

### 1. 依存ツールの配置

以下のツールを準備してください：

- `yt-dlp.exe`
- `ffmpeg.exe`
- `faster-whisper.exe`

`src-tauri/binaries/` に配置した実行ファイルはバンドルリソースとして優先利用されます。未配置の場合は設定画面の手動パス、環境変数、PATH の順に解決します。

### 2. 環境変数の設定

プロジェクトルートに `.env` ファイルを作成し、以下の設定を行います：

```env
# ffmpeg と faster-whisper.exe のパス
FFMPEG_PATH=C:\Users\makun\Faster-Whisper-XXL\ffmpeg.exe
WHISPER_PATH=C:\Users\makun\Faster-Whisper-XXL\faster-whisper.exe

# 作業ディレクトリ
TMP_DIR=D:\Document\Program\movie2text\tmp
OUTPUT_DIR=D:\Document\Program\movie2text\out
```

`.env.example` をコピーして編集することもできます。

設定変更の手順は [設定ガイド](/mnt/d/Document/Program/movie2text/docs/CONFIGURATION.md) にまとめています。
ビルドから実行までの流れは [ビルドと実行手順](/mnt/d/Document/Program/movie2text/docs/BUILD_AND_RUN.md) にまとめています。

### 3. 依存関係のインストール

```bash
npm install
```

### 4. 開発サーバーの起動

```bash
npm run tauri dev
```

### 5. 本番ビルド

```bash
npm run tauri build
```

ビルドされた実行ファイルは `src-tauri/target/release/` に生成されます。

## 使い方

1. **動画ファイルを選択**
   - 「動画ファイルを選択」ボタンをクリック
   - 変換したい動画ファイルを選択

2. **字幕を生成**
   - 「字幕を生成」ボタンをクリック
   - 処理ログがリアルタイムで表示されます

3. **出力フォルダを開く**
   - 変換完了後、「フォルダを開く」ボタンが有効になります
   - クリックするとエクスプローラーでSRTファイルが開きます

## 処理の流れ

1. **音声前処理（ffmpeg）**
   - 動画ファイルから音声を抽出
   - 16kHz モノラルのWAVファイルに変換
   - 一時ファイルは `TMP_DIR` に保存

2. **文字起こし（Whisper）**
   - faster-whisper.exe を使用
   - モデル: large-v3
   - 言語: 日本語
   - デバイス: CUDA
   - VAD（音声区間検出）有効
   - 出力形式: SRT
   - 出力先: `OUTPUT_DIR`

3. **字幕後処理（Premiere向け整形）**
   - 指定した最大文字数を超える字幕を複数の短いSRTエントリへ再分割
   - 句読点などの不要記号を除去しつつ、元の時間範囲内で開始・終了時刻を再配分

## 技術スタック

- **フロントエンド**: React + TypeScript + Vite
- **バックエンド**: Rust + Tauri 2.0
- **スタイリング**: CSS
- **音声処理**: ffmpeg
- **文字起こし**: faster-whisper (Whisper large-v3)

## プロジェクト構成

```
movie2text/
├── src/                    # Reactフロントエンド
│   ├── App.tsx            # メインコンポーネント
│   ├── main.tsx           # エントリーポイント
│   └── styles.css         # スタイル
├── src-tauri/             # Tauriバックエンド
│   ├── src/
│   │   ├── lib.rs         # アプリケーション初期化
│   │   ├── main.rs        # エントリーポイント
│   │   └── commands.rs    # Tauriコマンド実装
│   ├── Cargo.toml         # Rust依存関係
│   └── tauri.conf.json    # Tauri設定
├── tmp/                   # 一時ファイル（.gitignore）
├── out/                   # 出力SRTファイル（.gitignore）
├── docs/                  # ドキュメント
├── .env                   # 環境設定（.gitignore）
├── .env.example           # 環境設定テンプレート
└── package.json           # Node.js依存関係
```

## トラブルシューティング

### ビルドエラー

依存関係が正しくインストールされているか確認してください：

```bash
npm install
```

### 環境変数エラー

`.env` ファイルが正しく設定されているか確認してください。パスは絶対パスで指定する必要があります。

### CUDA関連エラー

- CUDA対応GPUがインストールされているか確認
- faster-whisper.exe が正しく動作するか確認

## ライセンス

このプロジェクトは個人使用を目的としています。

## 開発

### 開発環境

```bash
npm run dev          # Vite開発サーバー
npm run tauri dev    # Tauri開発モード
```

### ビルド

```bash
npm run build        # フロントエンドビルド
npm run tauri build  # アプリケーションビルド
```

## 貢献

バグ報告や機能リクエストは、GitHubのIssuesでお願いします。
