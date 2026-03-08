# ビルドと実行手順

このドキュメントは、Movie2Text を Windows で動かすまでの手順をまとめたものです。

## 前提

必要なもの:

- Windows 11
- Node.js と npm
- Rust / Cargo
- Visual Studio C++ Build Tools
- `ffmpeg.exe`
- `faster-whisper.exe`
- CUDA が使える NVIDIA GPU

Tauri 2 の Windows ビルドでは、Rust と Visual Studio 系ツールが必要です。

## 1. リポジトリを開く

```bash
cd /path/to/movie2text
```

## 2. 依存関係をインストールする

```bash
npm install
```

## 3. `.env` を設定する

プロジェクトルートに `.env` を置きます。テンプレートは `.env.example` です。

```env
FFMPEG_PATH=C:\\Tools\\ffmpeg\\ffmpeg.exe
WHISPER_PATH=C:\\Tools\\whisper\\faster-whisper.exe
TMP_DIR=D:\\movie2text\\tmp
OUTPUT_DIR=D:\\movie2text\\out
```

詳細は [設定ガイド](/mnt/d/Document/Program/movie2text/docs/CONFIGURATION.md) を参照してください。

## 4. 開発モードで実行する

フロントエンドだけを見る場合:

```bash
npm run dev
```

Tauri アプリ全体を起動する場合:

```bash
npm run tauri dev
```

実際には以下が順番に動きます。

- Vite 開発サーバー
- Rust バックエンドのビルド
- Tauri ウィンドウの起動

## 5. 本番ビルドを作る

フロントエンドのみビルド:

```bash
npm run build
```

アプリ全体をビルド:

```bash
npm run tauri build
```

`tauri.conf.json` では `beforeBuildCommand` に `npm run build` が入っているため、Tauri ビルド時にフロントエンドも先にビルドされます。

## 6. 生成物の場所

フロントエンド成果物:

- `dist/`

Tauri の実行ファイルやインストーラー:

- `src-tauri/target/release/`
- `src-tauri/target/release/bundle/`

通常は `bundle` 配下にインストーラーや配布用ファイルが作られます。

## 7. ビルド後に実行する

開発中:

- `npm run tauri dev` でそのまま起動します。

ビルド済みアプリを直接確認する場合:

- `src-tauri/target/release/` 配下の exe を実行します。

配布用として確認する場合:

- `src-tauri/target/release/bundle/` 配下のインストーラーを使います。

## 8. 動作確認の流れ

1. アプリを起動する
2. 動画ファイルを選ぶ
3. 「字幕を生成」を押す
4. ログ欄に `ffmpeg処理を開始します...` が出ることを確認する
5. `OUTPUT_DIR` に `.srt` が生成されることを確認する
6. 必要なら字幕編集画面で保存できることを確認する

## 9. よくある詰まりどころ

### `.env` 関連ですぐ失敗する

確認点:

- `.env` がプロジェクトルートにあるか
- `FFMPEG_PATH` / `WHISPER_PATH` が絶対パスか
- キー名が `OUTPUT_DIR` になっているか

### `npm run build` が Rollup まわりで失敗する

依存関係の optional package が欠けていることがあります。まず以下を試します。

```bash
npm install
```

それでも直らない場合は `node_modules` と lock file を入れ直して再インストールします。

### Tauri ビルドが通らない

確認点:

- Rust が入っているか
- `cargo --version` が通るか
- Visual Studio C++ Build Tools が入っているか

### 文字起こし開始直後に落ちる

確認点:

- `ffmpeg.exe` が単体で起動できるか
- `faster-whisper.exe` が単体で起動できるか
- CUDA ドライバが正しいか

## 10. コマンド一覧

```bash
npm install
npm run dev
npm run build
npm run tauri dev
npm run tauri build
```
