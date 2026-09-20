import {mkdirSync} from 'node:fs';
const pages=await(await fetch('http://127.0.0.1:9223/json/list')).json();
const ws=new WebSocket(pages.find(p=>p.type==='page').webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
let id=0;const pending=new Map();
ws.onmessage=e=>{const m=JSON.parse(e.data);if(pending.has(m.id)){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result);}};
const send=(method,params={})=>new Promise((resolve,reject)=>{const n=++id;pending.set(n,{resolve,reject});ws.send(JSON.stringify({id:n,method,params}));});
const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
const run=request=>evaluate(`window.__TAURI_INTERNALS__.invoke('dispatch',{request:${JSON.stringify(request)}})`);
mkdirSync('output/verification/offline',{recursive:true});
try{
  await send('Network.enable');
  await send('Network.emulateNetworkConditions',{offline:true,latency:0,downloadThroughput:0,uploadThroughput:0});
  const s=await run({op:'sample'}),g=s.project.groups.at(-1);
  const image=await run({op:'thumbnail',id:g.pages[0].id,width:400});
  if(!image.startsWith('data:image/png'))throw Error('Offline thumbnail failed');
  for(const format of ['pdf','docx','xlsx']){
    const result=await run({op:'prepare_export',format,quality:'standard',groups:[g.id]});
    await run({op:'finish_export',token:result.token,path:'C:/Development/projects/PDF-toolkit/output/verification/offline',overwrite:true});
  }
  await run({op:'remove_group',id:g.id});
  console.log('PASS: installed app, WebView network disabled, local rendering and PDF/DOCX/XLSX export');
}finally{
  await send('Network.emulateNetworkConditions',{offline:false,latency:0,downloadThroughput:-1,uploadThroughput:-1});
  ws.close();
}
