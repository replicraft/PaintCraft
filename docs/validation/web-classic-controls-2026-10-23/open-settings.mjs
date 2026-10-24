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
 for(const type of ['mousePressed','mouseReleased'])await command('Input.dispatchMouseEvent',{type,x:1334,y:45,button:'left',clickCount:1});
 await new Promise(r=>setTimeout(r,500));
 const screenshot=await command('Page.captureScreenshot',{format:'png'});
 fs.writeFileSync('/tmp/web-forms-settings.png',Buffer.from(screenshot.data,'base64'));
 console.log(JSON.stringify({exceptions}));
} finally {ws.close();}
