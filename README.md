# PDF Toolkit

Rust＋Tauriで動くWindows向けPDF整理アプリ。GUIはKoharuの配色、文字、余白、角丸を参照しています。文書処理はローカルで完結し、APIキーやサーバーは不要です。

## 機能

- 複数PDFの読み込み、サムネイル、拡大、並べ替え、回転、削除、Undo/Redo。
- Ctrl/Shift選択、範囲指定、ページ移動・コピー、グループ結合・分割・抽出。
- 日本語テキストコメントの追加・移動・サイズ変更・編集・削除。PDFへ標準FreeText注釈として保存。
- PDF、画像配置方式のDOCX/XLSXへの書き出しと変換結果の確認。
- Word/ExcelからPDFへの変換、Excelのシート指定（デスクトップ版Officeが必要）。
- 元PDFを含む単一 `.pdftk` 作業ファイルと、編集直後の自動保存・復旧。
- Koharuに合わせたライト・ダーク・システムテーマ。

1つの出力グループが1つのPDFになります。ページのダブルクリックでプレビューとコメント編集を開きます。内部ドラッグはポインター操作で処理し、外部PDFのファイルドロップと併用します。

## 利用と配布

Windows x64向けインストーラーはビルド後の `target/release/bundle/nsis/` に生成されます。PDFium DLL、日本語フォント、ライセンス、WebView2オフラインインストーラーを含む構成です。インストール後はNode.js、Rust、開発サーバーは不要です。

BasicのPDF編集・PDF→DOCX/XLSX出力はOffice不要。DOCX→PDFにはWord、XLSX→PDFにはExcelが必要です。現環境ではOffice 16.xで検証しています。署名・Store申請は後続工程です。

作業ファイルと復旧データには取り込んだPDFの解除済みコピーが含まれます。元の入力PDF・Office文書は上書きしません。復旧データは `%LOCALAPPDATA%/lab.dailyai.pdf-toolkit/recovery.pdftk` に保存します。

## 開発

### 追加機能の試験版

追加開発の検証ビルドでは、「書き出す」→PDFから圧縮モード、目標MB、開くパスワードを設定できます。前後比較でPDF・ページ・倍率を切り替え、表示した最終サイズを確認して保存します。目標未達の場合は了承のチェックが必要です。

パスワードは出力するPDFだけを保護します。`.pdftk` と復旧データは暗号化されず、秘密値も保存しません。保護設定のある作業を再開した場合はパスワードを再入力します。新しい作業形式はv2で、旧Basic版では開けません。旧v1作業の読み込みには対応しています。

現在のローカル検証実行ファイルは `target/debug/pdf-toolkit.exe`。通常版とは別IDの検証ビルドで、既存の復旧データと分離しています。配布インストーラーはまだ更新していません。ページ番号・透かし・Explorer結合は後続開発です。

### ビルドと検証

前提：Windows x64、Rust/MSVC、Visual Studio C++ Build Tools、Node.js、WebView2。

```powershell
powershell -ExecutionPolicy Bypass -File scripts/bootstrap.ps1
npm ci
npm run tauri dev
```

PDFiumはchromium/7881をSHA-256検証付きで取得します。フォントと依存ライセンスはリポジトリ内の `assets/` にあります。依存バージョンはCargo.lockとpackage-lock.jsonで固定しています。

```powershell
cargo test --workspace
node --experimental-strip-types --test scripts/test-output.mjs
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
npm run build
npm run tauri build
```

Officeがある開発PCでの統合試験：

```powershell
cargo run -p pdf-toolkit-core --example verify_office
```

依存更新後は `python scripts/collect-licenses.py` で同梱ライセンスを更新します。PythonおよびPyMuPDFは開発検証用で、製品の実行依存ではありません。

## 構成と資料

- `crates/core/`：ページモデル、PDF編集・描画、作業保存、OOXML生成、Officeアダプター。
- `src-tauri/`：Rustコマンド、Undo/Redo、進捗・キャンセル、自動保存、配布設定。
- `src/`：TypeScriptによるUI。
- [要件](01_Requirements.md)、[開発プロンプト](02_Development_Prompt.md)。
- [追加開発プロンプト：圧縮・ページ番号・透かし・パスワード・Explorer結合](03_Additional_Development_Prompt.md)（開発中：圧縮・暗号化・比較プレビューを検証ビルドへ統合済み。追加進捗25％。進捗は [追加開発進捗](docs/additional-progress.md)）。
- [実装計画](docs/implementation-plan.md)、[要件対応](docs/requirements-matrix.md)、[進捗](docs/progress.md)。
- [技術判断](docs/technical-decisions.md)、[検証結果と制限](docs/test-report.md)、[依存ライセンス](assets/licenses/THIRD_PARTY.md)。

`refinement.md` は未入手です。既存要件で実装を進める旨のユーザー指示を記録して着手しています。将来のBYOK・AI機能とWindows Store公開は今回の実装範囲に含めません。
