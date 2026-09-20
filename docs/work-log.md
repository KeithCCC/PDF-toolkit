# Work log

## 2026-09-20 — Basic版と追加開発プロンプトの保存

- Branch: `link`
- Rust＋TauriによるPDF整理、注釈、作業保存・復旧、DOCX/XLSX変換、Office入力を実装。Koharu準拠の日本語UIとNSIS配布を追加。
- 依存ロック、PDFium取得・ハッシュ検証、同梱ライセンス、試験スクリプト、要件対応と検証記録を保存。
- 圧縮・容量指定、ページ番号・透かし・パスワード、Explorer結合の追加開発プロンプトを作成。追加実装はこのコミット後に開始する。
- 検証：Basic版のRustテスト14件、fmt/clippy、TypeScript、release/NSIS、導入・起動・アンインストールは `docs/test-report.md` に記録。コミット前にRustテストとフロントエンドビルドを再実行。
- 制限：Office未導入の別OS、実プリンターなどの環境互換試験は未実施。配布バイナリ・生成出力・取得DLLは除外し、再生成手順をGit管理する。
