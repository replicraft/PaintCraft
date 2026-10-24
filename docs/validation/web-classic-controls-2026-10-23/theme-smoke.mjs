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
const reload=async()=>{const old=await evaluate('performance.timeOrigin');await command('Page.reload');for(let n=0;n<100;n++){await pause(100);if(await evaluate(`performance.timeOrigin !== ${old} && !!document.getElementById('loading')?.hidden`)){await pause(500);return;}}throw new Error('Reload did not start');};
try {
 await command('Runtime.enable');await command('Page.enable');
 await click(30,69);await pause(300);
 const afterTheme=await prefs();
 if(afterTheme?.theme!=='light')throw new Error(JSON.stringify({afterTheme}));
 await click(72,91);await click(66,113);await pause(300);
 const afterSkin=await prefs();
 if(afterSkin?.skin!=='v7')throw new Error(JSON.stringify({afterSkin}));
 await reload();
 let screenshot=await command('Page.captureScreenshot',{format:'png'});
 fs.writeFileSync('/tmp/web-forms-light-v7.png',Buffer.from(screenshot.data,'base64'));
 const reloaded=await prefs();const clean=!(await evaluate('window.paintcraft.is_dirty()'));
 if(reloaded.theme!=='light'||reloaded.skin!=='v7'||!clean)throw new Error('Preferences were not retained');
 await evaluate('(()=>{const key="paintcraft.web.preferences.v1";const prefs=JSON.parse(localStorage.getItem(key));prefs.skin="v11";localStorage.setItem(key,JSON.stringify(prefs));})()');
 await reload();
 await click(1126,164);await pause(400);
 screenshot=await command('Page.captureScreenshot',{format:'png'});
 fs.writeFileSync('/tmp/web-forms-color.png',Buffer.from(screenshot.data,'base64'));
 const result={afterTheme,afterSkin,reloaded,clean,exceptions};
 fs.writeFileSync('/tmp/web-forms-theme-check.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({theme:reloaded.theme,skin:reloaded.skin,clean,exceptions}));
} catch(error){const screenshot=await command('Page.captureScreenshot',{format:'png'});fs.writeFileSync('/tmp/web-forms-failure.png',Buffer.from(screenshot.data,'base64'));console.log(await prefs());throw error;}
finally{ws.close();}
