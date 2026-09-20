# PDF Toolkit Implementation Plan

Goal: Rust＋TauriによるローカルWindowsアプリを完成させる。
Spec: `01_Requirements.md`、`02_Development_Prompt.md`。2026-09-20の「実装を進めて」によりrefinement.mdなしの着手を承認済み。
Execution: このセッションで実装し、各フェーズの開始・完了をチャットで報告する。

## 構成

- `crates/core/src/model.rs`：ページ・グループ・コメントの編集。Project→Projectの操作をRustで検証。
- `crates/core/src/pdf.rs`：PDFiumで読み込み・描画・ページ複製。lopdfで編集可能なFreeText注釈と独立したAppearanceを生成。
- `crates/core/src/storage.rs`：入力PDFを内包するバージョン付きZIP作業ファイルと原子的保存。
- `crates/core/src/convert.rs`：画像配置OOXML生成・再読込プレビュー。
- `crates/core/src/office.rs`：専用PowerShell子プロセスでOffice COM操作、終了・キャンセル管理。
- `src-tauri/src/main.rs`：IPC、Rust側の作業状態・Undo/Redo・復旧・バックグラウンドジョブ。
- `src/main.ts`、`src/style.css`：Koharuを参照する日本語UI。表示と入力を担当。

## フェーズ

- [x] 1：Rustのページ操作テストを先に失敗させ、実装する。PDF DLLを固定して取り込み・コピー・注釈表示を実ファイルで確認。Tauriを起動する。
- [x] 2：実PDFの取り込み・描画・回転・書き出し・再読込を統合テスト。元ファイル不変を確認。
- [x] 3：選択・移動・コピー・結合・分割・Undo/Redo・名前検証を実装し、複数グループと境界ケースを確認。
- [x] 4：コメント座標とAppearanceを実装。日本語・回転・再編集・既存注釈保持を確認。
- [x] 5：作業保存・復旧。元ファイル移動後の再開、壊れた作業ファイル、保存中断を確認。
- [x] 6：DOCX/XLSX生成と内容に基づくプレビュー、Officeの個別検出・シート選択・変換。
- [x] 7：UIの実操作、型検査、Rust tests/clippy/fmt、release、インストール・起動を検証。

## 特に確認する境界

入力と同じ保存先、複数選択の自己移動、コピーのコメント独立性、回転済みPDFの座標、暗号化・破損PDF、キャンセル時の一時ファイル、Officeの既存プロセス保護。

## 判断記録

- 日本語FreeTextの見た目は注釈専用Appearanceへ画像として格納する案を検証する。本文へ焼き付けず、Contentsと編集情報を保持する。フォント差を抑え、再編集を維持するため。
- Web UIはTypeScript＋Vite。PDF編集ロジックはRustに置き、UIに二重の編集状態を持たせない。
