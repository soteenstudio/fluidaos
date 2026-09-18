import assert from 'node:assert/strict';
import {existsSync,readFileSync} from 'node:fs';
import {join} from 'node:path';
import test from 'node:test';

const ids=['files','editor','notes','terminal','monitor','calculator','settings','wallpapers','clock','calendar','photos','archive','app-center'];
const required=['manifest.json','index.html','app.js'];

test('every built-in source and registry package is complete',()=>{
 for(const id of ids)for(const file of required){
  assert.ok(existsSync(join('packages/apps',id,file)),`${id} source is missing ${file}`);
  assert.ok(existsSync(join('app_packages/builtin',id,file)),`${id} registry is missing ${file}`);
 }
});

test('built-in applications contain package-owned functional interfaces',()=>{
 for(const id of ids){
  const script=readFileSync(join('packages/apps',id,'app.js'),'utf8');
  assert.match(script,/type:'ready'/,`${id} does not complete the frame handshake`);
  assert.doesNotMatch(script,/Running in an isolated FluidaOS package/,`${id} is still a placeholder`);
  assert.match(script,/(onclick|onsubmit|onchange|call\()/,`${id} has no application behavior`);
 }
});

test('the shell exposes a retryable frame error without weakening the sandbox',()=>{
 const manager=readFileSync('src/client/shell/windowManager.ts','utf8');
 assert.match(manager,/class="application-error"/);
 assert.match(manager,/data-retry-app/);
 assert.match(manager,/sandbox="allow-scripts"/);
 assert.doesNotMatch(manager,/allow-same-origin/);
});

test('the Files frame runtime is attached before navigation and reused on retry',()=>{
 const manager=readFileSync('src/client/shell/windowManager.ts','utf8');
 const attach=manager.indexOf('const runtimeCleanup=attachApplicationRuntime');
 const navigate=manager.indexOf('frame.src=source',attach);
 assert.ok(attach>=0&&navigate>attach,'runtime must attach before iframe navigation');
 assert.doesNotMatch(manager,/<iframe[^>]+src=/,'new application frames must not have a src attribute');
 assert.match(manager,/onclick=\(\)=>\{waitForReady\(\);frame\.src=source\}/);
 assert.equal(manager.match(/attachApplicationRuntime\(/g)?.length,1,'retry must not attach another runtime');
 assert.match(manager,/runtime\.cleanup\(\)/,'closing a window must clean up its runtime');
 assert.match(manager,/const ready=\(\)=>\{[^}]*clearTimeout\(timer\)[^}]*error\.hidden=true;frame\.hidden=false/,'ready must clear the timeout and show the frame');
 assert.match(manager,/setTimeout\(\(\)=>\{frame\.hidden=true;error\.hidden=false\},4000\)/,'missing ready must show the error panel');
 for(const file of ['manifest.json','index.html','app.js'])assert.ok(existsSync(join('packages/apps/files',file)),`Files package is missing ${file}`);
});

test('application runtime initializes immediately and after every load',async()=>{
 const windowListeners=new Map();
 globalThis.location={href:'https://fluida.test/'};
 globalThis.window={
  addEventListener(type,listener){windowListeners.set(type,listener)},
  removeEventListener(type,listener){if(windowListeners.get(type)===listener)windowListeners.delete(type)}
 };
 const frameListeners=new Map(),messages=[];
 const contentWindow={postMessage(message){messages.push(message)}};
 const iframe={
  src:'https://fluida.test/api/apps/files/assets/index.html',contentWindow,
  addEventListener(type,listener){frameListeners.set(type,listener)},
  removeEventListener(type,listener){if(frameListeners.get(type)===listener)frameListeners.delete(type)}
 };
 const {attachApplicationRuntime}=await import('../dist/client/apps/apps.js');
 let ready=false;
 const cleanup=attachApplicationRuntime(iframe,{id:'files',capabilities:['files:read','files:write']},()=>{ready=true});
 assert.deepEqual(messages,[{fluida:true,type:'init',appId:'files'}]);
 frameListeners.get('load')();
 assert.equal(messages.length,2,'load must send init again');
 await windowListeners.get('message')({source:contentWindow,origin:'null',data:{fluida:true,appId:'files',type:'ready'}});
 assert.equal(ready,true,'valid ready message must reach the shell');
 cleanup();
 assert.equal(windowListeners.has('message'),false);
 assert.equal(frameListeners.has('load'),false);
 delete globalThis.window;
 delete globalThis.location;
});
