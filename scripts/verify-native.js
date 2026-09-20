(async () => {
  const run = (request) => window.__TAURI_INTERNALS__.invoke('dispatch', {request});
  const assert = (condition, message) => {if (!condition) throw Error(message);};
  const root = 'C:/Development/projects/PDF-toolkit/output/verification';
  let state = await run({op:'state'});
  for (const group of state.project.groups) await run({op:'remove_group',id:group.id});
  state = await run({op:'import',path:root+'/sample.pdf'});
  const page = state.project.groups[0].pages[0];
  state = await run({op:'comment',page:page.id,comment:{id:'',text:'日本語の確認\n保存・復元',x:30,y:110,width:210,height:80,font_size:16,color:'#202020',background:'#fff6c4'}});
  const comment = state.project.groups[0].pages[0].comments[0];
  state = await run({op:'comment',page:page.id,comment:{...comment,text:'日本語の確認\n再編集済み'}});
  state = await run({op:'extract',ids:[page.id],copy:true});
  const copied = state.project.groups[1].pages[0];
  await run({op:'delete_comment',page:copied.id,id:copied.comments[0].id});
  state = await run({op:'state'});
  assert(state.project.groups[0].pages[0].comments.length===1 && state.project.groups[1].pages[0].comments.length===0,'Independent copied comment');
  await run({op:'save_project',path:root+'/native/work.pdftk'});
  await run({op:'open_project',path:root+'/native/work.pdftk'});
  const protectedSource = await run({op:'prepare_export',format:'pdf',groups:[state.project.groups[0].id]});
  let rejected = false;
  try {await run({op:'finish_export',token:protectedSource.token,path:root,overwrite:true});} catch(error) {rejected=String(error).includes('元の入力');}
  assert(rejected,'Original PDF overwrite protection after reopening project');
  await run({op:'prepare_office',path:root+'/sample.docx',sheets:[]});
  state=await run({op:'accept_office'});
  const officeGroup=state.project.groups.at(-1);
  await run({op:'rename',id:officeGroup.id,name:'Office変換'});
  await run({op:'save_project',path:root+'/native/work.pdftk'});
  state=await run({op:'open_project',path:root+'/native/work.pdftk'});
  assert(state.project.groups.at(-1).pages[0].source_path.endsWith('sample.docx'),'Office original path retained');
  for(const format of ['pdf','docx','xlsx']) {
    const result=await run({op:'prepare_export',format,quality:'standard',groups:[]});
    assert(result.previews.length>=3,'Actual export previews');
    await run({op:'finish_export',token:result.token,path:root+'/native',overwrite:true});
  }
  let invalid=false;try{await run({op:'import',path:root+'/native/work.pdftk'});}catch{invalid=true;}
  assert(invalid,'Corrupt PDF is rejected');
  state=await run({op:'state'});assert(state.project.groups.length===3,'Failed input preserves project');
  return 'PASS: comment edit/delete/copy, standalone project, source protection, Office input, PDF/DOCX/XLSX export, error recovery';
})()
