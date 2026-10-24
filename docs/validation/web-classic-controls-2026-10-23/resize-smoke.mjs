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
const click=async(x,y)=>{await command('Input.dispatchMouseEvent',{type:'mouseMoved',x,y});for(const type of ['mousePressed','mouseReleased'])await command('Input.dispatchMouseEvent',{type,x,y,button:'left',clickCount:1});await pause(120);};
const key=async(key,modifiers=0)=>{for(const type of ['keyDown','keyUp'])await command('Input.dispatchKeyEvent',{type,key,modifiers});await pause(100);};
const type=async(x,y,text)=>{await click(x,y);await click(x,y);await key('a',4);await command('Input.insertText',{text});await key('Enter');};
const prefs=()=>evaluate('JSON.parse(localStorage.getItem("paintcraft.web.preferences.v1"))');
try {
  await command('Runtime.enable');await command('Page.enable');
  await type(622,386,'96');await type(622,429,'64');
  await click(548,647);await pause(400);
  const after=await prefs();const dirty=await evaluate('window.paintcraft.is_dirty()');
  if(after?.canvas?.last?.width!==96 || after?.canvas?.last?.height!==64 || !dirty)throw new Error(JSON.stringify({after,dirty}));
  await key('z',4);await pause(300);
  const undoClean=!(await evaluate('window.paintcraft.is_dirty()'));
  if(!undoClean)throw new Error('Undo did not return to the clean state');
  await command('Page.reload');
  for(let n=0;n<80;n++){await pause(100);if(await evaluate('!!document.getElementById("loading")?.hidden'))break;}
  await pause(700);
  const screenshot=await command('Page.captureScreenshot',{format:'png'});
  fs.writeFileSync('/tmp/web-forms-remembered.png',Buffer.from(screenshot.data,'base64'));
  const result={after,undoClean,ready:await evaluate('!!document.getElementById("loading")?.hidden'),cleanAfterReload:!(await evaluate('window.paintcraft.is_dirty()')),exceptions};
  fs.writeFileSync('/tmp/web-forms-resize-check.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
} catch(error){const screenshot=await command('Page.captureScreenshot',{format:'png'});fs.writeFileSync('/tmp/web-forms-failure.png',Buffer.from(screenshot.data,'base64'));console.log(await evaluate('({active:document.activeElement?.outerHTML,prefs:localStorage.getItem("paintcraft.web.preferences.v1")})'));throw error;}
finally{ws.close();}
