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
  await command('Page.enable');
  await command('Runtime.enable');
  await command('Emulation.setDeviceMetricsOverride', {width:1360, height:900, deviceScaleFactor:1, mobile:false});
  await command('Emulation.setEmulatedMedia', {features:[{name:'prefers-color-scheme',value:'light'}]});
  await command('Page.navigate', {url:'http://127.0.0.1:8088/'});
  let state;
  for (let attempt = 0; attempt < 90; ++attempt) {
    await new Promise(resolve => setTimeout(resolve, 500));
    const result = await command('Runtime.evaluate', {
      expression:'({ready:!!document.getElementById("loading")?.hidden, text:document.getElementById("loading")?.textContent, handle:!!window.paintcraft, webgpu:!!navigator.gpu})',
      returnByValue:true,
    });
    state = result.result.value;
    if (state?.ready || state?.text?.includes('could not start')) break;
  }
  if (!state?.ready) throw new Error(JSON.stringify({state, exceptions}));
  await new Promise(resolve => setTimeout(resolve, 1500));
  const screenshot = await command('Page.captureScreenshot', {format:'png'});
  fs.writeFileSync('/tmp/paintcraft-web-browser.png', Buffer.from(screenshot.data,'base64'));
  console.log(JSON.stringify({state,exceptions,screenshot:'/tmp/paintcraft-web-browser.png'}));
  if (exceptions.length) process.exitCode = 1;
} finally {
  ws.close();
}
