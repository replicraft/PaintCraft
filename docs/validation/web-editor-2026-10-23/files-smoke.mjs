import fs from 'node:fs';
const targets = await fetch('http://127.0.0.1:9229/json/list').then(r => r.json());
const target = targets.find(t => t.type === 'page');
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let id = 0;
const pending = new Map();
const exceptions = [];
ws.onmessage = event => {
  const message = JSON.parse(event.data);
  if (message.id) {
    const entry = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) entry.reject(new Error(JSON.stringify(message.error)));
    else entry.resolve(message.result);
  } else if (message.method === 'Page.javascriptDialogOpening') {
    command('Page.handleJavaScriptDialog',{accept:true}).catch(()=>{});
  } else if (message.method === 'Runtime.exceptionThrown') {
    exceptions.push(message.params.exceptionDetails);
  }
};
function command(method, params = {}) {
  return new Promise((resolve, reject) => {
    const next = ++id;
    pending.set(next, {resolve, reject});
    ws.send(JSON.stringify({id: next, method, params}));
  });
}
const pause = ms => new Promise(r=>setTimeout(r,ms));
const evaluate = async expression => (await command('Runtime.evaluate',{expression,returnByValue:true})).result.value;
const shortcut = async key => {
  await command('Input.dispatchKeyEvent',{type:'keyDown',key,code:'Key'+key.toUpperCase(),modifiers:4});
  await command('Input.dispatchKeyEvent',{type:'keyUp',key,code:'Key'+key.toUpperCase(),modifiers:4});
  await pause(300);
};
const waitFor = async predicate => {
  for(let n=0;n<100;n++) { const result=await predicate(); if(result) return result; await pause(100); }
  throw new Error('Timed out waiting for browser state');
};
const mouse = (type,x,y,extra={})=>command('Input.dispatchMouseEvent',{type,x,y,...extra});
const openFile = async file => {
  await shortcut('o');
  await waitFor(()=>evaluate('!!document.querySelector("#rfd-input")'));
  const {root} = await command('DOM.getDocument');
  const {nodeId} = await command('DOM.querySelector',{nodeId:root.nodeId,selector:'#rfd-input'});
  await command('DOM.setFileInputFiles',{nodeId,files:[file]});
  await evaluate('document.querySelector("#rfd-input")?.dispatchEvent(new Event("change",{bubbles:true}))');
  await waitFor(async()=> !(await evaluate('!!document.querySelector("#rfd-input")')));
  await pause(500);
};
try {
  await command('Runtime.enable'); await command('DOM.enable'); await command('Page.enable');
  await command('Page.setInterceptFileChooserDialog',{enabled:true});
  await command('Emulation.setDeviceMetricsOverride',{width:1360,height:900,deviceScaleFactor:1,mobile:false});
  await command('Page.navigate',{url:'http://127.0.0.1:8088/'});
  await waitFor(()=>evaluate('!!document.getElementById("loading")?.hidden'));
  await pause(500);
  fs.rmSync('/tmp/paintcraft-web-downloads/Untitled.pcnd',{force:true});
  fs.mkdirSync('/tmp/paintcraft-web-downloads',{recursive:true});
  await command('Browser.setDownloadBehavior',{behavior:'allow',downloadPath:'/tmp/paintcraft-web-downloads',eventsEnabled:true});
  await mouse('mouseMoved',450,430);
  await mouse('mousePressed',450,430,{button:'left',buttons:1,clickCount:1});
  await mouse('mouseMoved',620,460,{button:'left',buttons:1});
  await mouse('mouseReleased',620,460,{button:'left',buttons:0,clickCount:1});
  await pause(400);
  await shortcut('s');
  await waitFor(()=>evaluate('!!document.querySelector("dialog a[download]")'));
  const requestedName = await evaluate('document.querySelector("dialog a").download');
  await evaluate('document.querySelector("dialog a").click()');
  await waitFor(()=>fs.existsSync('/tmp/paintcraft-web-downloads/Untitled.pcnd'));
  await waitFor(async()=> !(await evaluate('!!document.querySelector("dialog")')));
  const stillDirty = await evaluate('window.paintcraft.is_dirty()');
  await shortcut('z');
  const cleanAfterUndo = !(await evaluate('window.paintcraft.is_dirty()'));
  await openFile('/tmp/web-editor-released.png');
  const pngClean = !(await evaluate('window.paintcraft.is_dirty()'));
  let screenshot = await command('Page.captureScreenshot',{format:'png'});
  fs.writeFileSync('/tmp/web-files-imported-png.png',Buffer.from(screenshot.data,'base64'));
  await openFile('/tmp/paintcraft-web-downloads/Untitled.pcnd');
  const projectClean = !(await evaluate('window.paintcraft.is_dirty()'));
  screenshot = await command('Page.captureScreenshot',{format:'png'});
  fs.writeFileSync('/tmp/web-files-reopened-project.png',Buffer.from(screenshot.data,'base64'));
  const result = {requestedName,downloadBytes:fs.statSync('/tmp/paintcraft-web-downloads/Untitled.pcnd').size,stillDirty,cleanAfterUndo,pngClean,projectClean,exceptions};
  console.log(JSON.stringify(result));
  fs.writeFileSync('/tmp/web-files-browser-check.json',JSON.stringify(result,null,2)+'\n');
  if(!stillDirty || !cleanAfterUndo || !pngClean || !projectClean || exceptions.length) process.exitCode=1;
} catch(error) {
  const screenshot=await command('Page.captureScreenshot',{format:'png'});fs.writeFileSync('/tmp/web-files-failure.png',Buffer.from(screenshot.data,'base64'));
  console.log(await evaluate('({dirty:window.paintcraft?.is_dirty(),active:document.activeElement?.outerHTML,body:document.body.innerText})'));throw error;
} finally { ws.close(); }
