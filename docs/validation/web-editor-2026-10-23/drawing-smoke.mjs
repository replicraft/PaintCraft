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
const pause = () => new Promise(r=>setTimeout(r,350));
const dirty = async () => (await command('Runtime.evaluate',{expression:'window.paintcraft.is_dirty()',returnByValue:true})).result.value;
const capture = async name => {
  await pause();
  const result = await command('Page.captureScreenshot',{format:'png',clip:{x:400,y:380,width:320,height:200,scale:1}});
  fs.writeFileSync(`/tmp/web-editor-${name}.png`,Buffer.from(result.data,'base64'));
  return result.data;
};
const mouse = (type,x,y,extra={})=>command('Input.dispatchMouseEvent',{type,x,y,...extra});
try {
  await command('Runtime.enable');
  const beforeDirty = await dirty();
  const before = await capture('blank');
  await mouse('mouseMoved',450,430);
  await mouse('mousePressed',450,430,{button:'left',buttons:1,clickCount:1});
  for(let n=1;n<=12;n++) { await mouse('mouseMoved',450+n*14,430+Math.sin(n/3)*35,{button:'left',buttons:1}); await new Promise(r=>setTimeout(r,20)); }
  const duringDirty = await dirty();
  const preview = await capture('held');
  await mouse('mouseReleased',618,430+Math.sin(4)*35,{button:'left',buttons:0,clickCount:1});
  const committed = await capture('released');
  const afterDirty = await dirty();
  await command('Input.dispatchKeyEvent',{type:'keyDown',key:'z',code:'KeyZ',modifiers:4});
  await command('Input.dispatchKeyEvent',{type:'keyUp',key:'z',code:'KeyZ',modifiers:4});
  const undone = await capture('undone');
  const undoneDirty = await dirty();
  const result = {beforeDirty,duringDirty,afterDirty,undoneDirty,changed:before!==committed,previewMatchesRelease:preview===committed,undoRestores:undone===before,exceptions};
  fs.writeFileSync('/tmp/web-editor-browser-check.json',JSON.stringify(result,null,2)+'\n');
  console.log(JSON.stringify(result));
  if(beforeDirty || !duringDirty || !afterDirty || undoneDirty || before===committed || preview!==committed || undone!==before || exceptions.length) process.exitCode=1;
} finally { ws.close(); }
