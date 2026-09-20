# 技術調査メモ

2026-09-20。設計・技術検証前の調査結果。採用候補の動作を保証する資料ではない。

## 固定条件

- Rust＋Tauri、Windowsで動くスタンドアロンBasic版。
- ローカル処理、独自サーバー・アカウント・AI不要。
- Office依存はDOCX/XLSXからPDFへの変換のみ。
- GUIのLook and FeelはKoharuに合わせる。

## Koharu参照

参照ファイル：`C:/Development/projects/rust-text-editor/src/styles.css`。

| 項目 | ライト | ダーク |
|---|---|---|
| アプリ背景 | #f5f5f5 | #202020 |
| ペイン背景 | #ffffff | #242424 |
| 文字 | #202020 | #f0f0f0 |
| 補助文字 | #666666 | #b0b0b0 |
| アクセント | #0969b8 | #75bfff |
| 選択背景 | #e1effa | #263d50 |
| 罫線 | #dedede | #3d3d3d |

UI文字13px、Interからsystem-ui・Segoe UIへフォールバック。ボタンは7px角丸、8px 12pxの余白、ウェイト600。フォーカス表示は2pxの輪郭線。Koharuにはシステム色を使うテーマ定義もある。外観の参照は行うが、Koharu固有のエディター機能をPDF Toolkitへ追加するものではない。

## PDFエンジン候補

`pdfium-render` の公式資料を確認。PDFiumへのRustバインディングを候補として調査する。ページ表示だけでなく、ページ複製、既存注釈保持、暗号化PDF、日本語FreeText注釈の生成・外観・再編集、DLL再配布まで実ファイルで検証した後に採否を決める。

- https://docs.rs/crate/pdfium-render/latest
- https://docs.rs/crate/pdfium-render/latest/source/README.md

型が存在することと、必要な日本語注釈の生成・表示が正常に動くことは別に確認する。実装バージョンとDLLバージョンの組み合わせを固定して記録する。

## Windows配布

Tauri公式のWindows配布資料ではMSI・NSISがサポートされ、WebView2のオフラインインストーラー同梱も設定できる。配布方式の候補はNSIS＋必要なPDFエンジンDLL。実際の配布ビルド・インストール検証後に確定する。

- https://v2.tauri.app/distribute/windows-installer/
- https://v2.tauri.app/reference/webview-versions/

インストール後のオフライン動作と、ネットワークなしでの初回インストール可否は別項目として検証する。

## 環境

Rust/Cargo 1.92.0、Node.js 22.20.0、npm 11.10.1。Word・ExcelのCOM登録あり。実際のOfficeバージョン・起動・変換は未検証。


## 採用した構成（2026-09-20）

- Tauri 2.11.6、TypeScript 5.9、Vite 6.4.3。実バージョンはロックファイルを正本とする。
- pdfium-render 0.9.4＋PDFium chromium/7881 x64を表示・ページ複製に使用。lopdf 0.45でFreeText/AP・UTF-16BE Contents・再編集JSONを保存。
- PDFium DLL SHA-256：79D4676B656CFB1ABCEA88F9ADE3B4B0826C5200382DB5F4EC72A636C598C118。取得アーカイブSHA-256はscripts/bootstrap.ps1。
- 暗号化PDFはPDFiumがパスワードを認証してから編集許可を検査。バインディングが認識しないR5/R6は暗号辞書のPビットでassembly/annotation権限を確認する。許可されない編集は拒否。
- 日本語コメント用にNoto Sans JPをOFLライセンス付きで同梱。OSフォントのインストール状況に依存しない。改行制御文字は描画対象から除外。
- 文書本文を画像化せず、コメント単位の注釈Appearanceだけを画像として格納。本アプリの再編集情報を独立保存。
- 作業形式はZIP（manifest.json version=1とsources/*.pdf）、拡張子.pdftk。入力PDFは解除済みのローカルコピー。編集ごとに復旧ファイルを一時保存・置換。復旧ファイル競合を避ける単一起動。
- 標準150dpi・高画質300dpi。レンダリング幅は最大5000px、高さ6000px。サムネイルは400px、拡大1400px、画面に入った時点で生成しキャッシュ件数を制限。
- DOCXは各ページごとにサイズを持つセクションとページ原点固定画像。XLSXは1ページ1シート、A4縦横判定・印刷1ページ設定。プレビューは生成したXMLを再読込して寸法・配置・画像参照から描画する。
- Office COMは専用PowerShell子プロセス。マクロ無効・読み取り専用で開く。HWNDから実PIDを特定し、既存PIDと区別、開始時刻も記録してキャンセル時に所有したOfficeだけを停止。180秒タイムアウト。
- Tauriの外部ファイルドロップを有効に保ち、アプリ内ドラッグはPointer Eventsで扱う。Windows OLEとHTML5ドラッグの競合を避ける。
- NSIS配布でPDFium・各依存ライセンス・WebView2オフラインインストーラーを同梱。NotoフォントはEXEへ埋め込む。

追加の公式参照：

- https://v2.tauri.app/plugin/single-instance/
- https://github.com/google/fonts/tree/main/ofl/notosansjp
- https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F7881
