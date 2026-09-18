import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync} from 'node:fs';
import {readFile} from 'node:fs/promises';
import {parseHTML} from 'linkedom';
import {bindDropdown,dropdown,nextListboxIndex,requestText} from '../dist/client/core/controls.js';

function installDom(body='<button id="previous">Previous</button>'){
 const {window}=parseHTML(`<!doctype html><html><body>${body}</body></html>`);
 for(const name of ['document','Node','Element','HTMLElement','HTMLInputElement','CustomEvent','MutationObserver'])globalThis[name]=window[name];
 let focused=null;Object.defineProperty(window.document,'activeElement',{configurable:true,get:()=>focused});window.HTMLElement.prototype.focus=function(){focused=this;};
 window.HTMLInputElement.prototype.select??=function(){};
 return window;
}

function key(window,target,key,options={}){const event=new window.Event('keydown',{bubbles:true,cancelable:true});Object.defineProperties(event,{key:{value:key},shiftKey:{value:Boolean(options.shiftKey)}});target.dispatchEvent(event);}

test('listbox keyboard navigation',()=>{
 assert.equal(nextListboxIndex(0,'ArrowDown',3),1);
 assert.equal(nextListboxIndex(2,'ArrowDown',3),0);
 assert.equal(nextListboxIndex(0,'ArrowUp',3),2);
 assert.equal(nextListboxIndex(1,'Home',3),0);
 assert.equal(nextListboxIndex(1,'End',3),2);
 assert.equal(nextListboxIndex(0,'ArrowDown',0),-1);
});

test('dropdown exposes linked ARIA state and preserves its value and change contract',()=>{
 const window=installDom(dropdown([{value:'name',label:'Name'},{value:'size',label:'Size'}],'name','Sort'));
 const root=document.querySelector('[data-dropdown]'),trigger=root.querySelector('[role="combobox"]'),list=root.querySelector('[role="listbox"]'),options=[...root.querySelectorAll('[role="option"]')];
 assert.equal(trigger.getAttribute('aria-controls'),list.id);
 assert.equal(list.getAttribute('aria-labelledby'),trigger.id);
 let bubbled=false,callbackValue;
 document.body.addEventListener('change',event=>{bubbled=true;assert.equal(event.detail.value,'size');});
 bindDropdown(root,value=>callbackValue=value);
 key(window,trigger,'ArrowDown');
 assert.equal(trigger.getAttribute('aria-expanded'),'true');
 assert.equal(list.hidden,false);
 assert.equal(document.activeElement===options[1],true);
 key(window,options[1],'Enter');
 assert.equal(root.value,'size');
 assert.equal(trigger.querySelector('span').textContent,'Size');
 assert.equal(options[1].getAttribute('aria-selected'),'true');
 assert.equal(trigger.getAttribute('aria-expanded'),'false');
 assert.equal(callbackValue,'size');
 assert.equal(bubbled,true);
});

test('dropdown supports pointer selection, Space, Escape, and cleanup',()=>{
 const window=installDom(dropdown([{value:'one',label:'One'},{value:'two',label:'Two'}],'one','Choice'));
 const root=document.querySelector('[data-dropdown]'),trigger=root.querySelector('[role="combobox"]'),list=root.querySelector('[role="listbox"]'),cleanup=bindDropdown(root);
 trigger.click();list.querySelector('[data-value="two"]').click();assert.equal(root.value,'two');assert.equal(trigger.querySelector('span').textContent,'Two');
 key(window,trigger,' ');
 assert.equal(list.hidden,false);
 key(window,list.querySelector('[role="option"]'),'Escape');
 assert.equal(list.hidden,true);
 cleanup();
 trigger.click();
 assert.equal(list.hidden,true);
});

test('requestText submits and restores focus',async()=>{
 const window=installDom(),previous=document.querySelector('#previous');previous.focus();
 const result=requestText('File name','draft.txt'),dialog=document.querySelector('[role="dialog"]'),input=dialog.querySelector('input');
 assert.equal(dialog.getAttribute('aria-labelledby'),dialog.querySelector('h2').id);
 assert.equal(dialog.querySelector('label').getAttribute('for'),input.id);
 input.value='final.txt';
 dialog.dispatchEvent(new window.Event('submit',{bubbles:true,cancelable:true}));
 assert.equal(await result,'final.txt');
 assert.equal(document.activeElement===previous,true);
});

test('requestText supports cancel, Escape, backdrop dismissal, and focus trapping',async()=>{
 const window=installDom(),previous=document.querySelector('#previous');previous.focus();
 let result=requestText('Cancel me');document.querySelector('[data-cancel]').click();assert.equal(await result,null);assert.equal(document.activeElement===previous,true);
 result=requestText('Escape me');key(window,document.querySelector('input'),'Escape');assert.equal(await result,null);
 result=requestText('Backdrop');document.querySelector('.control-modal').dispatchEvent(new window.Event('pointerdown',{bubbles:true}));assert.equal(await result,null);
 result=requestText('Trap focus');const input=document.querySelector('input'),submit=document.querySelector('[type="submit"]');submit.focus();key(window,submit,'Tab');assert.equal(document.activeElement===input,true);input.focus();key(window,input,'Tab',{shiftKey:true});assert.equal(document.activeElement===submit,true);document.querySelector('[data-cancel]').click();await result;
});

test('default prompts and selects are absent',async()=>{for(const path of ['../src/client/core/main.ts','../src/client/apps/apps.ts']){const source=await readFile(new URL(path,import.meta.url),'utf8');assert.doesNotMatch(source,/\bprompt\s*\(/);assert.doesNotMatch(source,/<select\b/i);}});

test('served control stylesheet contains the current dropdown and dialog design',async()=>{
 const source=await readFile(new URL('../src/client/styles/controls.css',import.meta.url),'utf8');
 for(const selector of ['.dropdown-list button:not([aria-selected=true]):before','.dropdown-trigger i','.dropdown-trigger[aria-expanded=true] i','.control-dialog-actions','.control-secondary','.control-primary'])assert.equal(source.includes(selector),true);
 assert.equal(existsSync(new URL('../src/public/styles/controls.css',import.meta.url)),false);
});
