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
