# 要件と実装・検証の対応

2026-09-20。`01_Requirements.md` を基準とする。最終確認の結果・制限は `test-report.md`。

| 要件 | 主な実装 | 検証 |
|---|---|---|
| 複数PDF・パスワード・元PDF不変 | `pdf.rs`、`main.rs` import、`main.ts` importPaths | 正常・破損・AES-256、入力先上書き拒否 |
| サムネイル・サイズ・拡大 | `main.ts` paper/loadPaper/preview | 実WebView2、遅延読み込み・キャッシュ上限 |
| 単一・Ctrl・Shift・範囲選択 | `main.ts` selection/range | UI操作、範囲入力検証 |
| 並べ替え・グループ間移動・コピー | `model.rs` transfer、UI pointer drag | 相対順・自己移動・コピーIDのテスト、実UI |
| 回転・削除・Undo/Redo | `model.rs`、`main.rs` checkpoint | 回転正規化、ネイティブIPCテスト |
| 結合・分割・抽出 | `model.rs`、`main.ts` | 複数グループの実操作・IPC |
| 名前・複数選択書き出し・上書き | validate_name、exportDialog、storage::write_file | 予約名、重複名、元入力保護、確認UI |
| 日本語コメントの追加・再編集・削除 | `pdf.rs` FreeText/AP、`main.ts` preview | 日本語内容、回転/CropBox、コピー独立性、入力失敗時保持 |
| コメントの他ビューア表示・印刷属性 | 注釈AppearanceとF=4 | PDFium＋MuPDFの独立レンダリング。実プリンターは未検証 |
| PDF→DOCX/XLSX | `convert.rs` | OOXML構造、Officeで再度PDFへ変換した2ページの表示 |
| Office不要の変換プレビュー | `convert.rs` preview_images | 生成XMLのページ寸法・関連画像・配置に基づく描画 |
| DOCX/XLSX→PDF・シート選択 | `office.rs`、`office.ps1` | Office 16.x、全体2ページ、選択シート1ページ |
| Word/Excel個別検出 | COM登録の個別照会・UIの拡張子制限 | 実COM検出＋検出結果3パターンを模擬したUI試験。未導入のクリーンOSは未検証 |
| Officeの隔離・終了・キャンセル | HWNDからPIDと開始時刻を記録 | 既存WINWORDを保持、所有プロセスのみ終了 |
| 作業ファイル保存・単体再開 | `storage.rs` ZIP形式v1 | 元PDF実体同梱、正常/破損、保存中断時の既存ファイル保持 |
| 自動保存・復元・破棄 | `main.rs` recovery、UI復旧ダイアログ | 強制終了と再起動、コメント復元・破棄 |
| 応答性・進捗・キャンセル | spawn_blocking、job-progress、AtomicBool | ページ単位PDF、1MB単位作業保存、Officeキャンセル |
| Koharu Look and Feel | `src/style.css` | Koharuソースから配色/余白/角丸/文字を参照、ライト/ダーク |
| スタンドアロン配布 | Tauri NSIS、PDFium、フォント、WebView2同梱 | release/NSIS、導入先実操作、通信無効時の出力、アンインストール。詳細はtest-report |

関連コードパスはリポジトリ内相対表記。Rustコアは `crates/core/src/`、Tauriは `src-tauri/src/main.rs`、UIは `src/`。
