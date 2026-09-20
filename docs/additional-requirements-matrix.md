# 追加要件対応

| 要件 | 実装先・予定 | 完了判定 | 状態 |
|---|---|---|---|
| 正確なMB指定・最終容量判定 | core/output.rs、pipeline.rs | 暗号化後の境界値、達成/未達、品質優先 | コア・実GUIで検証済み |
| 非破壊圧縮 | core/compression.rs、image_placement.rs | 文字・注釈保持、画像/透過/色空間、達成可能な試料 | DPI・Form・共有画像・100ページ合成試料まで検証済み |
| 番号・透かし | core/decorations.rs、model | 回転/CropBox/範囲/編集/保存/旧形式互換 | 未着手 |
| 開くパスワード | core/output.rs、出力UI | AES-256、別エンジンで正誤認証、非永続化 | 圧縮との統合済み、再開時再入力・MuPDF確認済み。装飾統合は後続 |
| 比較プレビュー・確認保存 | Tauri、src/output.ts | 同じ最終候補、未達了承、設定変更後無効化 | 実GUIで検証済み |
| 作業形式・設定保存 | core/model.rs、storage.rs | v1移行、v2出力設定、秘密値なし | 圧縮・保護フラグは検証済み。装飾は後続 |
| Explorer結合 | Shellブリッジ、merge_request、merge_queue | 実複数選択・受付・順序確認・既存作業保持 | 調査中 |
| NSIS登録・解除 | 配布フック | 導入・更新・アンインストール、長いパス | 未着手 |
| 既存機能回帰 | 既存tests＋統合試験 | 元入力保護、コメント再編集、Office、復旧 | 自動回帰・PDF/DOCX/XLSX出力成功。全機能完成後の再試験は後続 |
