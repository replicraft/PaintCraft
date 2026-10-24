import fs from 'node:fs';
const targets = await fetch('http://127.0.0.1:9229/json/list').then(r => r.json());
const target = targets.find(t => t.type === 'page' && t.url.startsWith('http://127.0.0.1:8088/'));
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
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const evaluate=async expression=>(await command('Runtime.evaluate',{expression,returnByValue:true})).result.value;
const click=async(x,y)=>{await command('Input.dispatchMouseEvent',{type:'mouseMoved',x,y});await pause(100);await command('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',buttons:1,clickCount:1});await pause(100);await command('Input.dispatchMouseEvent',{type:'mouseReleased',x,y,button:'left',buttons:0,clickCount:1});await pause(250);};
const key=async(key,modifiers=0)=>{const virtualKey={Home:36,End:35,Enter:13,Escape:27}[key]??key.toUpperCase().charCodeAt(0);const code=key.length===1?'Key'+key.toUpperCase():key;for(const type of ['keyDown','keyUp'])await command('Input.dispatchKeyEvent',{type,key,code,modifiers,windowsVirtualKeyCode:virtualKey});await pause(100);};
const type=async(x,y,text)=>{await click(x,y);await key('Home');await key('End',8);await command('Input.insertText',{text});await key('Enter');};
const prefs=()=>evaluate('JSON.parse(localStorage.getItem("paintcraft.web.preferences.v1"))');
const reload=async()=>{const old=await evaluate('performance.timeOrigin');await command('Page.reload');for(let n=0;n<100;n++){await pause(100);if(await evaluate(`performance.timeOrigin !== ${old} && !!document.getElementById('loading')?.hidden`)){await pause(500);return;}}throw new Error('Reload did not start');};
try {
 await command('Runtime.enable');await command('Page.bringToFront');await command('Emulation.setFocusEmulationEnabled',{enabled:true});await command('Page.enable');
 await key('Escape');await click(1126,164);await pause(500);
 await type(800,112,'#2080FF');await pause(500);
 await click(1004,492);await click(484,698);await pause(400);
 const after=await prefs();const clean=!(await evaluate('window.paintcraft.is_dirty()'));
 if(after?.custom_colors?.[9]!=='#2080FFFF'||after.custom_colors.filter(Boolean).length!==1||!clean)throw new Error(JSON.stringify({after,clean}));
 await reload();
 const persisted=await prefs();
 await click(1126,164);await click(680,470);await pause(400);
 const dirtyAfterPaint=await evaluate('window.paintcraft.is_dirty()');
 let screenshot=await command('Page.captureScreenshot',{format:'png'});
 fs.writeFileSync('/tmp/web-forms-color-persisted.png',Buffer.from(screenshot.data,'base64'));
 if(!dirtyAfterPaint)throw new Error('Color drawing did not publish');
 await key('z',4);await pause(200);
 const cleanAfterUndo=!(await evaluate('window.paintcraft.is_dirty()'));
 if(!cleanAfterUndo)throw new Error('Color undo failed');
 const saved=await evaluate('localStorage.getItem("paintcraft.web.preferences.v1")');
 await evaluate('localStorage.setItem("paintcraft.web.preferences.v1","{bad")');await reload();
 const invalidRetained=await evaluate('localStorage.getItem("paintcraft.web.preferences.v1")==="{bad"');
 const invalidReady=await evaluate('!!document.getElementById("loading")?.hidden && !window.paintcraft.is_dirty()');
 screenshot=await command('Page.captureScreenshot',{format:'png'});
 fs.writeFileSync('/tmp/web-forms-invalid-preferences.png',Buffer.from(screenshot.data,'base64'));
 await evaluate(`localStorage.setItem("paintcraft.web.preferences.v1",${JSON.stringify(saved)})`);
 const {identifier}=await command('Page.addScriptToEvaluateOnNewDocument',{source:'Object.defineProperty(window,"localStorage",{get(){throw new DOMException("denied","SecurityError")}})'});
 await reload();
 const deniedReady=await evaluate('!!document.getElementById("loading")?.hidden && !window.paintcraft.is_dirty()');
 screenshot=await command('Page.captureScreenshot',{format:'png'});
 fs.writeFileSync('/tmp/web-forms-denied-preferences.png',Buffer.from(screenshot.data,'base64'));
 await command('Page.removeScriptToEvaluateOnNewDocument',{identifier});await reload();
 const result={after,persisted,clean,dirtyAfterPaint,cleanAfterUndo,invalidRetained,invalidReady,deniedReady,exceptions};
 fs.writeFileSync('/tmp/web-forms-color-storage-check.json',JSON.stringify(result,null,2)+'\n');
 console.log(JSON.stringify({customSlot10:persisted.custom_colors[9],clean,dirtyAfterPaint,cleanAfterUndo,invalidRetained,invalidReady,deniedReady,exceptions}));
 if(!invalidRetained||!invalidReady||!deniedReady||exceptions.length)process.exitCode=1;
} catch(error){const screenshot=await command('Page.captureScreenshot',{format:'png'});fs.writeFileSync('/tmp/web-forms-failure.png',Buffer.from(screenshot.data,'base64'));console.log(await evaluate('({active:document.activeElement?.outerHTML})'));throw error;}
finally{ws.close();}
