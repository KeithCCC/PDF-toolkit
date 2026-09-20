// Integration check against an explicitly launched debug Tauri WebView2 (port 9223).
// Run only against this project's disposable sample session.
import {writeFileSync} from 'node:fs';
const pages = await (await fetch('http://127.0.0.1:9223/json/list')).json();
const target = pages.find(p => p.type === 'page');
const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject;});
let serial=0;const waiting=new Map();
socket.onmessage=e=>{const m=JSON.parse(e.data);if(waiting.has(m.id)){const {resolve,reject}=waiting.get(m.id);waiting.delete(m.id);m.error?reject(Error(JSON.stringify(m.error))):resolve(m.result);}};
const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++serial;waiting.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));});
const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
const run=async request=>{for(let n=0;n<100;n++){try{return await evaluate(`window.__TAURI_INTERNALS__.invoke('dispatch',{request:${JSON.stringify(request)}})`);}catch(e){if(!String(e).includes('別の処理が実行中'))throw e;await new Promise(r=>setTimeout(r,100));}}throw Error('IPC did not settle');};
const assert=(condition,message)=>{if(!condition)throw Error(message);};
const settled=async()=>{for(let n=0;n<100;n++){if(await evaluate('document.querySelector("#busy").hidden'))return;await new Promise(r=>setTimeout(r,100));}throw Error('UI did not settle');};
const point=selector=>evaluate(`(()=>{const e=document.querySelector(${JSON.stringify(selector)});e.scrollIntoView({block:'center'});const r=e.getBoundingClientRect();return {x:r.x+Math.min(r.width/2,60),y:r.y+Math.min(r.height/2,60)}})()`);
const mouse=(type,point,extra={})=>send('Input.dispatchMouseEvent',{type,...point,...extra});
try{
  await evaluate('document.querySelector("#dialog").close()');
  await settled();
  const before=await run({op:'state'});
  const first=before.project.groups[0].pages[0];
  const from=await point('.page-card');const to=await point('.end-drop');
  await mouse('mouseMoved',from);
  await mouse('mousePressed',from,{button:'left',buttons:1,clickCount:1,modifiers:2});
  await mouse('mouseMoved',to,{button:'left',buttons:1,modifiers:2});
  await mouse('mouseReleased',to,{button:'left',buttons:0,clickCount:1,modifiers:2});
  await settled();
  let state=await run({op:'state'});const after=state.project.groups[0].pages;
  assert(after.length===before.project.groups[0].pages.length+1,'Ctrl + pointer drag must copy');
  assert(after.at(-1).id!==first.id && after.at(-1).index===first.index,'Copied identity and source');
  const p=await point('.page-card');
  await mouse('mousePressed',p,{button:'left',buttons:1,clickCount:2});
  await mouse('mouseReleased',p,{button:'left',buttons:0,clickCount:2});
  await evaluate('document.querySelector("#comment-text").value="確認済み\\n日本語コメント";document.querySelector("#c-y").value="120";document.querySelector("#comment-form").requestSubmit()');
  await settled();
  state=await run({op:'state'});
  assert(state.project.groups[0].pages[0].comments.length===first.comments.length+1,'Comment form applied');
  await evaluate('document.querySelector("#dialog").close(); document.querySelector("#theme").value="dark";document.querySelector("#theme").dispatchEvent(new Event("change"))');
  const dark=await send('Page.captureScreenshot',{format:'png'});writeFileSync('output/verification/ui-final-dark.png',Buffer.from(dark.data,'base64'));
  await evaluate('document.querySelector("#theme").value="light";document.querySelector("#theme").dispatchEvent(new Event("change"))');
  const light=await send('Page.captureScreenshot',{format:'png'});writeFileSync('output/verification/ui-final-light.png',Buffer.from(light.data,'base64'));
  console.log('PASS: trusted Ctrl-drag copy, double-click preview, Japanese comment form, light/dark screenshots');
}finally{socket.close();}
