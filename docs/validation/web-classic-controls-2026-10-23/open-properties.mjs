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
try {
  await command('Runtime.enable');
  await command('Input.dispatchKeyEvent',{type:'keyDown',key:'e',code:'KeyE',modifiers:4});
  await command('Input.dispatchKeyEvent',{type:'keyUp',key:'e',code:'KeyE',modifiers:4});
  await new Promise(resolve=>setTimeout(resolve,700));
  const screenshot=await command('Page.captureScreenshot',{format:'png'});
  fs.writeFileSync('/tmp/web-forms-properties.png',Buffer.from(screenshot.data,'base64'));
  console.log(JSON.stringify({exceptions,screenshot:'/tmp/web-forms-properties.png'}));
} finally {ws.close();}
