# Movie2Text

動画ファイルから自動で文字起こしを行い、SRTファイルを生成するWindows用デスクトップアプリケーション

## 概要

Premiere Proで編集済みの動画ファイルをローカルで選択し、Whisper（faster-whisper.exe）を使って文字起こしし、SRTファイルを出力するTauri + React製のデスクトップアプリです。

## 主な機能

- 動画ファイルの選択（mp4, avi, mov, mkv, flv, wmv）
- ffmpegによる音声前処理（16kHz mono変換）
- Whisper (large-v3)による高精度な日本語文字起こし
- リアルタイムログ表示
- 生成されたSRTファイルをワンクリックで開く

## 必要な環境

- Windows 11
- CUDA対応GPU（Whisper処理用）
- ffmpeg.exe
- faster-whisper.exe

## セットアップ

### 1. 依存ツールの配置

以下のツールを準備してください：

- `ffmpeg.exe`
- `faster-whisper.exe`

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

2. **変換開始**
   - 「変換開始」ボタンをクリック
   - 処理ログがリアルタイムで表示されます

3. **SRTファイルを開く**
   - 変換完了後、「SRTファイルを開く」ボタンが有効になります
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
