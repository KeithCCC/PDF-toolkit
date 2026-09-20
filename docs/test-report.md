# 検証記録

更新：2026-09-20。Basic版の現PCでの検証記録。

## 環境と成功した検証

- Windows x64、Rust 1.92.0、Node.js 22.20.0、Office COM 16.x（Word/Excel）。
- `cargo test -p pdf-toolkit-core`：14件成功。
- `npm run tauri build -- --debug --no-bundle`：TypeScript、Vite、Tauriビルド成功。
- `cargo run -p pdf-toolkit-core --example verify_office`：DOCX/XLSXをOfficeでPDFへ戻し、各2ページ・各ページに画像内容があることを確認。Excel選択シートは1ページ。キャンセル後、既存Word PID 31720は存続、作成したExcel/Wordの残留なし。
- 実ネイティブWebView2のCDPを通した `scripts/verify-native.js`：コメント再編集/削除/コピー独立性、作業保存再開後の入力PDF上書き拒否、Office元パス保持、3形式の出力、失敗した入力からの継続。
- 編集済みの実アプリをStop-Processで強制終了し、起動後の「復元する」で2ページとコメント内容を復元。
- 二重起動時、後から起動したプロセスが終了。
- UIコメント幅をページ外の900ptにしたところ拒否され、本文の入力は保持、ダイアログ内に理由を表示。
- PDFiumと独立したMuPDFでFreeTextの日本語Contents、印刷フラグ、Appearanceを確認。

## 証跡

`output/verification/`（Git管理外）にスクリーンショット、出力PDF、DOCX/XLSX、Office再変換結果を格納する。自動試験用の小さなPDFは `crates/core/tests/fixtures/`。生成器は `scripts/make-fixtures.py`（開発検証用PyMuPDFのみ）。

## 検証範囲の制限

- クリーンな別PC、Office未導入OS、実プリンターでの印刷は未実施。
- Office互換性はこのPCのOffice 16.xで確認。保護ビューや組織ポリシーでCOMが制限される場合は変換エラーを表示する。
- DOCX/XLSX出力プレビューは、このアプリが生成する画像配置OOXMLに限定した描画。任意のOfficeファイルを解釈するビューアではない。Office入力は実Officeが作成したPDFを表示する。
- コメントはFreeText注釈として独立保存し、見た目は注釈Appearance内の画像で統一する。本アプリの再編集情報とUnicode本文を持つ。他製品での文字編集互換までは保証しない。
- 全編集直後に復旧用作業ファイルを保存。非常に大きな原稿では保存待ちが生じる。キャンセルはPDFのページ間、作業保存の1MB区間、Office専用処理の監視点で確認する。ネイティブAPIの1回の呼び出し途中は即時中断しない。
- バッチ出力を途中でキャンセルした場合、既に正常保存したグループのファイルは残る。各ファイルは一時ファイルからの原子的保存。
- 復旧・作業ファイルは解除済みPDFを含み、暗号化しない。元の入力ファイルは書き換えない。
- 署名およびMicrosoft Store申請・公開は今回の対象外。


## 最終ビルド・配布確認

- `cargo fmt --all -- --check`：成功。
- `cargo test -p pdf-toolkit-core`：14件成功、失敗0。
- `cargo clippy --workspace --all-targets -- -D warnings`：成功。
- `npm run tauri build`：TypeScript/Vite/release/NSIS成功。
- `scripts/bootstrap.ps1`：PDFiumの再取得・アーカイブSHA-256検証成功。
- `scripts/verify-ui.mjs`：ネイティブWebView2へ信頼されたマウス入力を送り、Ctrlドラッグのコピーと独立ID、ダブルクリック、コメントフォーム、ライト/ダークを確認。通常移動も実マウス入力で順序[0,1]→[1,0]を確認。
- `scripts/verify-office-gating.mjs`：製品UIソースをTypeScriptから変換し、検出結果とダイアログのみ模擬。Word/Excel両方なしでは無効、Wordのみではdocx、Excelのみではxlsxに制限。これはOffice未導入OSの実機試験とは区別する。
- 元サンプルPDFを別名へ移動したまま.pdftkを開き、同梱PDFからサムネイルを描画できた。
- 100ページの軽量ベクター試験PDF：debug版の取り込み＋自動保存96ms。高画質DOCX変換を開始して50ms後にキャンセル、処理中断と後続応答を確認。スキャン大容量PDFの性能を代表する数値ではない。
- NSISインストーラーを `output/installed` へサイレント導入、終了コード0。導入したEXE（22,704,128 bytes）とPDFium DLL、ライセンスを確認。
- 導入先を作業ディレクトリとし、PATHからNode.js/Cargo等を外して起動。開発サーバー1420番ポートは未起動。そのEXEでverify-native.jsが成功。
- `scripts/verify-offline.mjs`：導入先アプリのWebViewネットワークを無効化し、サムネイルとPDF/DOCX/XLSX出力に成功。OS全体のネットワーク切断やWebView2未導入OSでの初回導入を検証したものではない。
- 導入したuninstall.exe /S：終了コード0、導入先EXE削除を確認。検証用の復旧データもアプリの破棄操作で削除済み。

配布物：`target/release/bundle/nsis/PDF Toolkit_0.1.0_x64-setup.exe`

サイズ：226,221,446 bytes（約216MiB）。WebView2オフラインインストーラーを含む。

SHA-256：`D85ED076DF168A22D9254F9AA40FA3AD84002A38E76325F7D26CC7B8F829345B`

署名は行っていない。クリーンOSでのオフライン初回導入、Office未導入OS、実プリンター、異なるOfficeバージョンでの互換性は追加検証対象。現PCでの機能・配布検証の成功と区別する。
