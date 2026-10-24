import fs from 'node:fs';
const targets = await fetch('http://127.0.0.1:9229/json/list').then(r => r.json());
const ws = new WebSocket(targets.find(t => t.type === 'page').webSocketDebuggerUrl);
await new Promise(resolve => ws.onopen = resolve);
let id=0; const pending=new Map();
ws.onmessage=e=>{const m=JSON.parse(e.data); if(m.id){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result);}};
const command=(method,params={})=>new Promise((resolve,reject)=>{const n=++id;pending.set(n,{resolve,reject});ws.send(JSON.stringify({id:n,method,params}));});
try {
  await command('Input.dispatchMouseEvent',{type:'mousePressed',x:30,y:69,button:'left',clickCount:1});
  await command('Input.dispatchMouseEvent',{type:'mouseReleased',x:30,y:69,button:'left',clickCount:1});
  await new Promise(resolve=>setTimeout(resolve,500));
  const shot=await command('Page.captureScreenshot',{format:'png'});
  fs.writeFileSync('/tmp/paintcraft-web-browser-light.png',Buffer.from(shot.data,'base64'));
  console.log('Browser theme interaction captured.');
}finally{ws.close();}
