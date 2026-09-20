export type PdfSettings = { mode: "off" | "quality" | "balanced" | "small" | "target"; target_mb: string; protect: boolean };
export type OutputReport = { original_bytes: number; final_bytes: number; target_bytes: number | null; met_target: boolean | null; attempts: number; setting: string; warnings: string[]; protected: boolean };
export type ExportResult = { token: string; names: string[]; previews: string[]; reports: OutputReport[]; pageCounts: number[] };
const esc = (s: unknown) => String(s ?? "").replace(/[&<>"']/g,c=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"})[c]!);
export function validateOutputInput(settings: PdfSettings, password: string, confirmation: string) {
  if (settings.mode === "target" && (!/^\d+(\.\d{1,6})?$/.test(settings.target_mb.trim()) || !Number.isFinite(Number(settings.target_mb)) || Number(settings.target_mb) <= 0)) throw Error("目標サイズは小数点以下6桁までの正のMB数で指定してください。");
  if (settings.protect && (!password || password !== confirmation)) throw Error("開くパスワードと確認欄に同じ値を入力してください。");
  if (settings.protect && new TextEncoder().encode(password).length > 127) throw Error("パスワードはUTF-8で127 bytesまでです。");
}
export function outputFields(settings: PdfSettings) {
  return `<fieldset id="pdf-options"><legend>PDFの出力設定</legend><p class="hint">選択したグループすべてに以下の設定を適用します。1 MB = 1,000,000 bytes。</p><label>圧縮<select id="compression">${([['off','オフ'],['quality','画質優先'],['balanced','バランス'],['small','容量優先'],['target','目標サイズ指定']] as const).map(([v,l])=>`<option value="${v}" ${settings.mode===v?'selected':''}>${l}</option>`).join('')}</select></label><div id="target-options"><label>目標サイズ (MB)<input id="target-mb" type="number" min="0.000001" step="any" value="${esc(settings.target_mb)}" list="target-presets"></label><datalist id="target-presets"><option value="1"></option><option value="5"></option><option value="10"></option></datalist></div><label class="check"><input id="protect-pdf" type="checkbox" ${settings.protect?'checked':''}>開くパスワードを設定する（AES-256）</label><div id="password-options"><label>パスワード<input id="output-password" type="password" autocomplete="new-password"></label><label>パスワードの確認<input id="output-password-confirm" type="password" autocomplete="new-password"></label><label class="check"><input id="show-output-password" type="checkbox">パスワードを表示</label><p class="hint">保護されるのは書き出すPDFです。作業ファイルと復旧データは暗号化されません。パスワードは保存されず、次回は再入力が必要です。</p></div></fieldset>`;
}
export function bindOutputFields() {
  const input = (id: string) => document.getElementById(id) as HTMLInputElement;
  const secrets = [input('output-password'), input('output-password-confirm')];
  document.getElementById('dialog')!.addEventListener('close',()=>secrets.forEach(field=>{field.value='';}),{once:true});
  const refresh = () => {
    const pdf = input('format').value === 'pdf';
    document.getElementById('pdf-options')!.hidden = !pdf;
    document.getElementById('office-quality')!.hidden = pdf;
    document.getElementById('target-options')!.hidden = input('compression').value !== 'target';
    document.getElementById('password-options')!.hidden = !input('protect-pdf').checked;
  };
  for (const id of ['format','compression','protect-pdf']) input(id).addEventListener('change',refresh);
  input('show-output-password').onchange = () => {
    for(const id of ['output-password','output-password-confirm']) input(id).type = input('show-output-password').checked ? 'text' : 'password';
  };
  refresh();
}
export function readOutputFields() {
  const input = (id: string) => document.getElementById(id) as HTMLInputElement;
  const settings: PdfSettings = {mode:input('compression').value as PdfSettings['mode'],target_mb:input('target-mb').value,protect:input('protect-pdf').checked};
  const password = input('output-password').value;
  validateOutputInput(settings,password,input('output-password-confirm').value);
  return { settings, password:settings.protect ? password : undefined };
}
export function outputSummary(report: OutputReport) {
  const bytes = (n: number) => `${(n/1_000_000).toFixed(3)} MB（${n.toLocaleString()} bytes）`;
  const rate = report.original_bytes ? 100 * (1-report.final_bytes/report.original_bytes) : 0;
  return `<p>変更前：${bytes(report.original_bytes)} → 最終：<strong>${bytes(report.final_bytes)}</strong><br>${rate>=0?'削減率':'増加率'}：${Math.abs(rate).toFixed(1)}％${report.target_bytes===null?'':` ／ 目標：${bytes(report.target_bytes)} ／ <strong>${report.met_target?'目標達成':'目標未達'}</strong>`}<br>${esc(report.setting)} ／ ${report.attempts}候補を確認${report.protected?' ／ パスワード保護あり':''}</p>${report.warnings.length?`<details><summary>変更しなかった画像・注意事項</summary><ul>${report.warnings.map(w=>`<li>${esc(w)}</li>`).join('')}</ul></details>`:''}`;
}
