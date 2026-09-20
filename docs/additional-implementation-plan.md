# Additional PDF Features Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:executing-plans or superpowers:subagent-driven-development to implement the approved plan task-by-task. This kickoff implements the first independently testable core slice; unchecked tasks remain pending.

**Goal:** 既存Basic版へ圧縮・目標容量、番号・透かし・パスワード、Explorer結合を追加する。
**Architecture:** 出力候補の作成・暗号化・容量判定をRustコアへ分離し、同じ最終バイト列をプレビュー検証と保存に使う。装飾は作業設定、パスワードは出力処理だけの引数、Shell依頼は作業とは別の確認待ちキューとする。
**Tech Stack:** Rust、Tauri、PDFium、lopdf 0.45、TypeScript。追加の実行時CLIを利用者へ要求しない。
**Spec:** `03_Additional_Development_Prompt.md`。ユーザーは2026-09-20にコミット・プッシュ後の開発開始を指示済み。

## Global Constraints

- Rust＋Tauri、Koharuの見た目、オフライン動作を維持。
- 1MB＝1,000,000 bytes。最終暗号化後の実サイズで目標判定。
- 元入力は不変。ページ全体の画像化で圧縮を代替しない。
- パスワードは作業・復旧・ログ・コマンドラインに残さない。
- 追加開発進捗は既存Basic版と別。全フェーズ完了時だけ100％。
- 起動中のユーザーのテストアプリを強制終了しない。

## Review Focus

- 暗号化の増分で目標容量を超える候補を成功表示しない（Task 1）。
- マスク画像・色空間・注釈Appearanceを画像圧縮で壊さない（Task 2）。
- 範囲指定済み装飾を結合・分割したときIDと連番が一致する（Task 3）。
- 正しい日本語パスワードを他ビューアでも使用できる（Task 1/4）。
- Shellの複数依頼の誤結合・重複配送で既存作業を壊さない（Task 5/6）。

## Task 1: 出力候補・目標容量・AES-256のコア

Files: `crates/core/src/output.rs`, `crates/core/src/lib.rs`, `crates/core/tests/output.rs`, `crates/core/Cargo.toml`。
Interfaces: `TargetSize::from_mb(&str) -> Result<TargetSize>`、`Candidate { bytes, quality_rank }`、`choose_candidate(Vec<Candidate>, Option<TargetSize>) -> Result<Selection>`、`encrypt_pdf(&[u8], &str) -> Result<Vec<u8>>`。

- [x] 正確なMB→bytes、ゼロ・負数・過剰精度・桁あふれ、品質優先の達成候補と最小未達候補をテストし、未実装による失敗を確認。
- [x] 暗号化後だけサイズ超過になる境界、誤パスワード、空パスワード、日本語、暗号辞書V5/R6/256bitsを実ファイルで確認するテストを書く。
- [x] 既存lopdfのAES-256 APIとOS乱数を使用し、メモリ内の秘密値のみで生成する。所有者鍵は毎回独立した乱数とし、空・固定値にしない。
- [x] `cargo test -p pdf-toolkit-core --test output` とPDFium/MuPDFの独立検証を実行。GUIには未統合であることを記録。

## Task 2: 非破壊の圧縮候補

Files: `crates/core/src/compression.rs`, `crates/core/tests/compression.rs`。
Interfaces: `optimize_lossless(&[u8]) -> Result<Vec<u8>>`、画像候補作成は同じ入力から毎回独立して実施し、変更画像数・未対応画像の理由を返す。

- [x] 不要オブジェクトとストリーム圧縮を行ってもページ数・検索文字・注釈が保持される実PDFテストを先に作る。
- [x] 第一段階は8bit DeviceRGBの通常画像のJPEG再圧縮を検証。マスク・透過・Decode・特殊色空間・注釈参照を除外するテストを作る。
- [x] DPI調整はページ内配置行列を考慮して設計。共有画像は最大必要解像度を採用し、Forms・多重配置の上限を検証してから対応範囲を広げる。
- [x] 最大8候補、各候補を暗号化した後Task 1で評価。原稿より増量する場合と達成不能を区別し、ページ数と検索テキストを再検証。
- [x] 100ページのスキャン試料で時間・メモリ・キャンセルを測定。

## Task 3: 装飾と作業形式

Files: `crates/core/src/decorations.rs`, `crates/core/src/model.rs`, `crates/core/src/storage.rs`、各対応tests。

- [ ] ページ番号の開始値・表紙除外・分母・回転/CropBox位置のテストを先に追加。
- [ ] ページIDを対象とするグループ設定、文字透かし、番号をモデルに追加。本文と別の描画として生成。
- [ ] v1→新形式の既定値、秘密値を含まない保存、結合先設定継承、分割・抽出時のID写像、Undo/Redoをテスト・実装。
- [ ] 日本語・混在ページ・DOCX/XLSX反映・二重描画なしを実ファイルで検証。

## Task 4: 出力UI・最終候補の保存

Files: `src-tauri/src/main.rs`, `src/main.ts`, `src/style.css`。必要な新UIは `src/output.ts` に分離。

- [x] 出力状態のテスト：設定変更で候補無効化、未達の了承なしでは保存不可、保護設定だけ復旧しても秘密値なしで出力不可。
- [ ] 圧縮・装飾・開くパスワードの設定と、同一ページ・倍率での比較プレビューを実装。
- [x] Rustの候補キャッシュに最終バイト列を保管し、再変換せず保存する。平文一時データとキャンセルの後処理を検証。
- [ ] 独立ビューアと本アプリで暗号化出力を開き、全既存機能の回帰を実行。

## Task 5: Shell依頼の受け取り

Files: `crates/core/src/merge_request.rs`, `crates/core/tests/merge_request.rs`, `src-tauri/src/merge_queue.rs`。

- [ ] 依頼ID・配列パスの検証、依頼の重複排除、別依頼の分離、処理中のキューをテスト。
- [ ] 初回引数と単一起動コールバックを同じ受付に接続。利用者が確認するまではプロジェクトを変更しない。
- [ ] 自然順ソート・順序変更・エラー一覧・パスワード入力を結合画面に追加。未保存作業保持を検証。

## Task 6: 実Explorer連携・配布

Files: Windows Shellブリッジ用の独立crate、`src-tauri/tauri.conf.json`、NSISの登録・解除フック、`scripts/` の検証補助。

- [ ] IObjectWithSelection/IShellItemArrayを受け取れる方式を技術検証。単純な%1登録を複数選択対応として採用しない。
- [ ] 選択一覧を1依頼として渡す最小ブリッジと、長いパス・100ファイルでも切れないIPCを実装。PDF処理をShell内で行わない。
- [ ] ユーザー単位・任意選択の登録、更新時の冪等性、所有登録だけの解除を実装。
- [ ] Windows 11の従来メニューから2/20/100ファイルを実際に渡し、起動済み・処理中・別依頼・アンインストールを検証。
- [ ] NSIS、同梱ライセンス、README、追加テスト報告を更新。新releaseと旧版更新の両方を確認。

各タスクで失敗するテスト→実装→テスト成功を記録する。着手時のコミット・プッシュは `a747cfa`。追加開発分のコミットは独立した変更単位で扱う。
