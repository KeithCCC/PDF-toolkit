# 追加開発テスト記録

実施日：2026-09-20。基点：`a747cfa`。対象は初期のRustコア実装で、追加機能全体の受入完了ではない。

## 自動検証

| 検証 | 結果 |
|---|---|
| `cargo test -p pdf-toolkit-core` | 28件成功（既存14件＋追加14件） |
| `cargo clippy -p pdf-toolkit-core --all-targets -- -D warnings` | 成功 |
| `cargo fmt --all -- --check` | 成功 |
| `cargo check -p pdf-toolkit` | 成功 |

出力・圧縮テストは未実装での失敗を確認してから実装した。Unicodeのsoft hyphenだけからなるパスワードについて、空へ正規化されるため従来の非空チェックを通る回帰テストの失敗を確認。暗号化直後の空パスワード認証を検査する修正後、成功を確認した。

## 実PDFによる検証

- Decimal MB、1 byte境界、ゼロ・桁あふれ・不正形式、品質優先選択、達成不能、暗号化の容量増分を検証。
- PDF暗号辞書のV=5、R=6、暗号フィルタAESV3を確認。PDFiumで正しい日本語パスワードのみ受理し、毎回の出力が乱数によって変わることを確認。
- RGB画像入り2ページ＋日本語FreeText注釈の試料で、本文検索文字・ページ数・注釈を保持。Mask付き画像は変更せず理由を返す。キャンセル・不正品質値も検証。
- 圧縮＋暗号化試料は入力2,366,536 bytesから最終441,304 bytesへ縮小し、1,000,000 bytesの目標を達成。この結果はテスト試料の実測値であり、任意のPDFでの達成を保証しない。
- 独立したMuPDFで正誤パスワード、2ページ、検索テキスト、日本語注釈Contentsを確認。レンダリング画像 `output/verification/additional-compressed.png` も目視し、本文・注釈・画像の表示を確認。

検証用PDFと画像は `output/verification/` 配下の生成物で、Git管理しない。試料中のパスワードはテスト専用。

## 2026-09-21：GUI統合・回帰

Windows上の実Tauri/WebView2アプリ（検証専用ID `lab.dailyai.pdf-toolkit.verification`）を使用。`scripts/verify-additional.mjs` が通常アプリIDを拒否し、検証用作業だけを操作する。

- `cargo test --workspace`：通常41件成功。100ページ性能テスト1件は通常実行ではignore、releaseテストバイナリで別途実行して成功。
- `cargo clippy --workspace --all-targets -- -D warnings`、fmt、`npm run build`、デバッグアプリのビルド成功。
- `node --experimental-strip-types --test scripts/test-output.mjs`：2件成功。
- ネイティブ統合試験：目標0.1MBに対し暗号化後約71.1KBを達成。1 byte目標は約23.6KBで未達として扱い、了承なしの保存を拒否。
- 全2ページの比較画像、確認したbytesと保存ファイルサイズの一致、編集後の古いトークン拒否、保護設定を持つ作業の再開時にパスワードが必要なことを確認。
- 実アプリからキャンセルし、保存済み出力のSHA-256が変わらないことを確認。DOCX/XLSXへの出力も成功。
- 独立したMuPDFで暗号化出力を認証、2ページと本文検索文字、日本語注釈を確認。ZIP内manifestを調べ、v2の保護フラグがあり秘密値・passwordフィールドがないことを確認。
- 前後比較・未達了承の画面を `output/verification/additional/comparison.png` で目視。

### 不具合の再現と修正

- パスワードなしのlopdf読み込みでは暗号辞書しか取得できず、後からdecryptしてもプレビューにページがないことを再現。読み込み時に認証を渡す修正で描画成功。
- 注釈 `/P` がページツリー全体を圧縮除外にする問題を再現。保護参照の走査をPage/Pagesで止め、本文画像だけを圧縮できることを確認。
- ExtGStateのSMaskグループ内画像が変換される問題を、間接参照・直接辞書の両方で再現。マスク参照を保護対象に追加してサンプル保持を確認。
- 最終検証をページごとにキャンセル可能なPDFium描画検証へ変更。誤認証・ページ数不一致・途中キャンセルをテスト。
- PDFiumの個別FFI呼び出し間で別スレッドの処理が混ざる問題を、12スレッドの誤認証＋読み込み＋書き出し＋描画で再現。Engine操作全体の排他制御後、並列回帰を含む全テストが成功。
- CMYK、Decode付き画像のサンプル保持、共有画像の最大配置、UserUnit、回転を含むForm行列、元解像度を超えない縮小を確認。

### 100ページ性能試験

`hundred_page_scan_benchmark` をreleaseプロファイルで実行。100個の独立した1024×768画像XObjectを持つ合成試料（絵柄は繰り返し）。ページ番号の検索テキストと全100ページを維持。

| 入力 | 最終出力 | 処理時間 | ピークWorking Set | 候補 |
|---:|---:|---:|---:|---:|
| 117,259,716 bytes | 32,106,221 bytes | 5.41秒 | 632,516,608 bytes | 3 |

時間はコアの候補生成、メモリは試料生成を含むテストプロセスをWindowsで測定。実スキャン原稿の多様性やアプリ全体のピークは別途測定が必要。記録は `output/verification/100-page-performance.json` と `100-page-memory.json`。

## 現在の制限（2026-09-21更新）

- GUI統合は検証ビルドに含む。リリース実行ファイル・インストーラーは未更新。
- 画像再圧縮は8bit DeviceRGBに限定。透明度・特殊色空間・注釈等は保持。Type3フォント・Pattern・解析困難な配置ではDPI縮小を行わず理由を表示。
- 番号・透かし、Explorer連携と登録・解除は未実装。
- 同じ出力設定の再適用でもUndo履歴が増える軽微な指摘を記録。機能上の重要指摘は修正済み。
- 実ユーザー文書や別のWindows環境での統合受入は未実施。
