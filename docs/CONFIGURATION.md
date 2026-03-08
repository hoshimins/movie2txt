# 設定ガイド

Movie2Text で変更する設定は、大きく分けて 2 種類あります。

- `.env`
  Rust バックエンドが読む設定です。`ffmpeg.exe` や `faster-whisper.exe` の場所、作業ディレクトリ、出力先をここで管理します。
- `localStorage`
  React フロントエンドが読む設定です。現在は「1行あたりの最大文字数」のみを保存します。

## 1. `.env` を変更する

プロジェクトルートの `.env` を編集します。テンプレートは `.env.example` です。

```env
# ffmpeg と faster-whisper.exe のパス
FFMPEG_PATH=C:\\Tools\\ffmpeg\\ffmpeg.exe
WHISPER_PATH=C:\\Tools\\whisper\\faster-whisper.exe

# 作業ディレクトリ
TMP_DIR=D:\\movie2text\\tmp
OUTPUT_DIR=D:\\movie2text\\out
```

### 各項目の意味

- `FFMPEG_PATH`
  動画から音声を抽出する `ffmpeg.exe` の絶対パスです。
- `WHISPER_PATH`
  字幕を生成する `faster-whisper.exe` の絶対パスです。
- `TMP_DIR`
  中間の WAV ファイルを置く作業ディレクトリです。
- `OUTPUT_DIR`
  生成された `.srt` を保存するディレクトリです。

### 変更手順

1. `.env.example` をコピーして `.env` を作成するか、既存の `.env` を開きます。
2. すべて絶対パスで記述します。
3. Windows パスは `C:\\path\\to\\file.exe` のように `\\` を 2 つ重ねるか、`C:/path/to/file.exe` のように `/` を使います。
4. 保存後、アプリを再起動します。

### 注意点

- 実装側は `OUTPUT_DIR` を参照します。`OUT_DIR` では反映されません。
- `TMP_DIR` と `OUTPUT_DIR` は存在しなくても、実行時に作成されます。
- パスが間違っていると、書き起こし開始直後にエラーになります。

## 2. 文字数制限を変更する

1 行あたりの最大文字数は UI から変更します。値はブラウザ相当の保存領域である `localStorage` に入ります。

### UI から変更する方法

1. アプリを起動します。
2. 左側の「出力ルール」にある「1行あたりの最大文字数」を変更します。
3. 入力した値は自動で保存され、次回起動時にも引き継がれます。

### 動作ルール

- `0`
  自動分割なしです。
- `1` 以上
  字幕テキストを指定文字数ごとに分割します。

### 保存先

コード上では `src/App.tsx` で `localStorage["maxLineWidth"]` を読み書きしています。

## 3. Tauri アプリ設定を変更する

アプリ名、ウィンドウサイズ、バンドル設定などは `src-tauri/tauri.conf.json` を編集します。

よく触る例:

- ウィンドウタイトル
- 初期サイズ
- アイコン
- バンドル識別子

変更後は `npm run tauri dev` を再起動するか、必要に応じて `npm run tauri build` で再ビルドしてください。

## 4. 変更後に確認すること

- `npm run tauri dev` でアプリが起動する
- 動画選択後に書き起こしが始まる
- ログ欄に `ffmpeg処理を開始します...` が出る
- SRT が `OUTPUT_DIR` に生成される

## 5. よくある設定ミス

- `.env` のキー名を間違える
- 相対パスを書く
- `WHISPER_PATH` が別の exe 名になっている
- パスに日本語や空白があり、実ファイルの場所と一致していない
- `.env` を編集したあとにアプリを再起動していない
