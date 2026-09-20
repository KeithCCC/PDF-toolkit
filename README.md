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

前提：Windows x64、Rust/MSVC、Visual Studio C++ Build Tools、Node.js、WebView2。

```powershell
powershell -ExecutionPolicy Bypass -File scripts/bootstrap.ps1
npm ci
npm run tauri dev
```

PDFiumはchromium/7881をSHA-256検証付きで取得します。フォントと依存ライセンスはリポジトリ内の `assets/` にあります。依存バージョンはCargo.lockとpackage-lock.jsonで固定しています。

```powershell
cargo test -p pdf-toolkit-core
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
- [追加開発プロンプト：圧縮・ページ番号・透かし・パスワード・Explorer結合](03_Additional_Development_Prompt.md)（追加機能は未実装）。
- [実装計画](docs/implementation-plan.md)、[要件対応](docs/requirements-matrix.md)、[進捗](docs/progress.md)。
- [技術判断](docs/technical-decisions.md)、[検証結果と制限](docs/test-report.md)、[依存ライセンス](assets/licenses/THIRD_PARTY.md)。

`refinement.md` は未入手です。既存要件で実装を進める旨のユーザー指示を記録して着手しています。将来のBYOK・AI機能とWindows Store公開は今回の実装範囲に含めません。
