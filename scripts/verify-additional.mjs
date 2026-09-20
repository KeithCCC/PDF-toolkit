// Run only against the separately identified .verification debug application.
import {mkdirSync,writeFileSync,statSync,copyFileSync,readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
const root='C:/Development/projects/PDF-toolkit/output/verification/additional';
mkdirSync(root,{recursive:true});
let pages=[];
for(let attempt=0;attempt<100;attempt++) {
  try { pages=await(await fetch('http://127.0.0.1:9223/json/list')).json(); if(pages.some(p=>p.type==='page'))break; } catch {}
  await new Promise(r=>setTimeout(r,100));
}
const target=pages.find(p=>p.type==='page');
const ws=new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
let id=0; const pending=new Map();
ws.onmessage=e=>{const m=JSON.parse(e.data); if(pending.has(m.id)){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}};
const send=(method,params={})=>new Promise((resolve,reject)=>{const n=++id;pending.set(n,{resolve,reject});ws.send(JSON.stringify({id:n,method,params}));});
const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
const run=async request=>{for(let n=0;n<200;n++){try{return await evaluate(`window.__TAURI_INTERNALS__.invoke('dispatch',{request:${JSON.stringify(request)}})`);}catch(e){if(!String(e).includes('別の処理が実行中'))throw e;await new Promise(r=>setTimeout(r,100));}}throw Error('IPC busy timeout');};
const assert=(c,m)=>{if(!c)throw Error(m);};
const waitFor=async expression=>{for(let n=0;n<400;n++){if(await evaluate(expression))return;await new Promise(r=>setTimeout(r,100));}throw Error('Timeout '+expression);};
try {
  await waitFor('document.title.includes("PDF Toolkit") && !!window.__TAURI_INTERNALS__ && !!document.querySelector("#busy")');
  // Verify app identity before any project mutation; never target the user's normal session.
  const appIdentifier=await evaluate("window.__TAURI_INTERNALS__.invoke('plugin:app|identifier')");
  assert(appIdentifier==='lab.dailyai.pdf-toolkit.verification','Refuse to alter non-test app');
  await waitFor('document.querySelector("#busy").hidden');
  await evaluate('document.querySelector("#dialog").close()');
  let s=await run({op:'state'});
  for(const g of s.project.groups)await run({op:'remove_group',id:g.id});
  s=await run({op:'import',path:'C:/Development/projects/PDF-toolkit/output/verification/additional-original.pdf'});
  const g=s.project.groups.at(-1);
  await run({op:'rename',id:g.id,name:'圧縮・暗号化テスト'});
  await run({op:'set_output_settings',groups:[g.id],settings:{mode:'target',target_mb:'0.1',protect:true}});
  let rejected=false;
  try{await run({op:'prepare_export',format:'pdf',groups:[g.id]});}catch{rejected=true;}
  assert(rejected,'Missing secret must not disable protection');
  const result=await run({op:'prepare_export',format:'pdf',groups:[g.id],password:'検証-only-password'});
  assert(result.reports[0].met_target===true,'100KB target must be met for image fixture');
  for(const index of [0,1]) {
    const images=await run({op:'output_preview',token:result.token,group:0,index,width:700});
    assert(images.before.startsWith('data:image/png')&&images.after.startsWith('data:image/png'),'Compare every page');
  }
  await run({op:'finish_export',token:result.token,path:root,overwrite:true});
  assert(statSync(root+'/'+result.names[0]).size===result.reports[0].final_bytes,'Saved bytes match measured bytes');
  copyFileSync(root+'/'+result.names[0],root+'/protected-final.pdf');
  await run({op:'save_project',path:root+'/protected-settings.pdftk'});
  await run({op:'open_project',path:root+'/protected-settings.pdftk'});
  rejected=false;try{await run({op:'prepare_export',format:'pdf',groups:[g.id]});}catch{rejected=true;}
  assert(rejected,'Reopened protected project must ask for password');
  for(const format of ['docx','xlsx']) {
    const office=await run({op:'prepare_export',format,groups:[g.id],quality:'standard'});
    await run({op:'finish_export',token:office.token,path:root,overwrite:true});
    assert(statSync(root+'/'+office.names[0]).size>0,'Office output regression');
  }
  await run({op:'set_output_settings',groups:[g.id],settings:{mode:'target',target_mb:'0.000001',protect:false}});
  const protectedHash=createHash('sha256').update(readFileSync(root+'/protected-final.pdf')).digest('hex');
  const cancelResult=await evaluate(`(async()=>{
    const operation=window.__TAURI_INTERNALS__.invoke('dispatch',{request:{op:'prepare_export',format:'pdf',groups:[${JSON.stringify(g.id)}]}}).then(()=>({cancelled:false}),e=>({cancelled:String(e).includes('キャンセル')}));
    await new Promise(resolve=>setTimeout(resolve,100));
    await window.__TAURI_INTERNALS__.invoke('cancel');
    return await operation;
  })()`);
  assert(cancelResult.cancelled,'Native cancellation is observed');
  assert(createHash('sha256').update(readFileSync(root+'/protected-final.pdf')).digest('hex')===protectedHash,'Cancellation leaves saved output unchanged');
  const unmet=await run({op:'prepare_export',format:'pdf',groups:[g.id]});
  assert(unmet.reports[0].met_target===false,'Unmet reported');
  rejected=false;try{await run({op:'finish_export',token:unmet.token,path:root,overwrite:true});}catch{rejected=true;}
  assert(rejected,'No implicit save of unmet target');
  await run({op:'finish_export',token:unmet.token,path:root,overwrite:true,acknowledgeUnmet:true});
  const stale=await run({op:'prepare_export',format:'pdf',groups:[g.id]});
  await run({op:'rotate',ids:[g.pages[0].id],delta:90});
  rejected=false;try{await run({op:'finish_export',token:stale.token,path:root,overwrite:true,acknowledgeUnmet:true});}catch{rejected=true;}
  assert(rejected,'Edit invalidates cached candidate');
  await run({op:'undo'});
  // Renderer was empty while IPC fixtures were created. Use its sample action to
  // refresh through a real UI operation, then test output settings normally.
  await evaluate('document.querySelector("#dialog").close(); if(document.querySelector("[data-action=sample]"))document.querySelector("[data-action=sample]").click();else document.querySelector("[data-action=undo]").click()');
  await waitFor('!!document.querySelector(".page-card") && document.querySelector("#busy").hidden');
  await evaluate('document.querySelector("[data-action=export]").click()');
  await waitFor('!!document.querySelector("#compression")');
  await evaluate(`document.querySelector('#compression').value='target';document.querySelector('#compression').dispatchEvent(new Event('change'));document.querySelector('#target-mb').value='0.000001';document.querySelector('#protect-pdf').checked=false;document.querySelector('#protect-pdf').dispatchEvent(new Event('change'));document.querySelector('#prepare-export').click()`);
  await waitFor('!!document.querySelector("#compare-after")?.src');
  assert(await evaluate('document.querySelector("#finish-export").disabled'),'Unmet UI disables save');
  await evaluate('document.querySelector("#acknowledge-unmet").click()');
  assert(!await evaluate('document.querySelector("#finish-export").disabled'),'Acknowledgment enables save');
  await evaluate('document.querySelector("#compare-page").value="2";document.querySelector("#compare-page").dispatchEvent(new Event("change"))');
  await run({op:'state'});
  const screenshot=await send('Page.captureScreenshot',{format:'png'});
  writeFileSync(root+'/comparison.png',Buffer.from(screenshot.data,'base64'));
  writeFileSync(root+'/result.json',JSON.stringify({report:result.reports[0],unmet:unmet.reports[0],checks:'password/reopen/exact size/2-page preview/unmet acknowledgment/stale output/UI'},null,2));
  console.log('PASS: native compression/protection/preview/save/migration/stale cache and UI acknowledgment');
  await run({op:'discard_recovery'});
} finally {ws.close();}
