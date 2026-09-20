// Explicitly simulated detection results; this is not a clean-OS Office test.
const pages=await(await fetch('http://127.0.0.1:9223/json/list')).json();
const ws=new WebSocket(pages.find(p=>p.type==='page').webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
let id=0;const pending=new Map();
ws.onmessage=e=>{const m=JSON.parse(e.data);if(pending.has(m.id)){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result);}};
const send=(method,params={})=>new Promise((resolve,reject)=>{const n=++id;pending.set(n,{resolve,reject});ws.send(JSON.stringify({id:n,method,params}));});
const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));return r.result.value;};
const {readFileSync}=await import('node:fs');
const ts=await import('typescript');
const source=ts.transpileModule(readFileSync('src/main.ts','utf8'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText.replace(/^import .*;$/gm,'').replace(/^export \{\};$/gm,'');
try{
  for(const [available,expected] of [[{word:false,excel:false},[]],[{word:true,excel:false},['docx']],[{word:false,excel:true},['xlsx']]]){
    const expression=`(async()=>{
      const invoke=async(cmd,args)=>{if(cmd==='dispatch'&&args.request.op==='detect')return ${JSON.stringify(available)};if(cmd==='dispatch'&&args.request.op==='recovery_exists')return false;throw Error('Unexpected IPC in simulated test');};
      const isTauri=()=>true;const listen=async()=>()=>{};const getCurrentWindow=()=>({onDragDropEvent:async()=>{},onCloseRequested:async()=>{}});
      let captured=[];const open=async options=>{captured=options.filters[0].extensions;return null;};const save=async()=>null;const confirm=async()=>false;
      ${source}
      const button=document.querySelector('[data-action=office]');
      await action('office',button);
      return {disabled:button.disabled,formats:captured,title:button.title};
    })()`;
    const result=await evaluate(expression);
    if(result.disabled!==(expected.length===0)||JSON.stringify(result.formats)!==JSON.stringify(expected))throw Error(JSON.stringify(result));
    console.log('PASS simulated detection:',available,'UI:',result);
  }
}finally{await send('Page.reload');ws.close();}
