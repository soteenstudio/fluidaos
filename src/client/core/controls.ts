export interface DropdownOption { value:string;label:string }

const escapeHtml=(value:string)=>value.replace(/[&<>"']/g,character=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[character]!));
let controlId=0;
const nextControlId=(kind:string)=>`fluida-${kind}-${++controlId}`;

export function dropdown(options:DropdownOption[],value:string,label:string,attributes=''){
 const selected=options.find(option=>option.value===value)??options[0],triggerId=nextControlId('select'),listId=nextControlId('listbox');
 return `<div class="custom-dropdown" data-dropdown data-value="${escapeHtml(selected?.value??'')}" ${attributes}><button id="${triggerId}" type="button" class="dropdown-trigger" role="combobox" aria-label="${escapeHtml(label)}" aria-haspopup="listbox" aria-controls="${listId}" aria-expanded="false"><span>${escapeHtml(selected?.label??'')}</span><i aria-hidden="true"></i></button><div id="${listId}" class="dropdown-list" role="listbox" aria-labelledby="${triggerId}" hidden>${options.map(option=>`<button type="button" role="option" data-value="${escapeHtml(option.value)}" aria-selected="${option.value===selected?.value}">${escapeHtml(option.label)}</button>`).join('')}</div></div>`;
}

export function nextListboxIndex(current:number,key:string,length:number){
 if(!length)return-1;
 if(key==='Home')return 0;if(key==='End')return length-1;
 if(key==='ArrowDown')return(current+1+length)%length;
 if(key==='ArrowUp')return(current-1+length)%length;
 return current;
}

export function bindDropdown(root:HTMLElement,onChange?:(value:string)=>void){
 const trigger=root.querySelector<HTMLButtonElement>('[role="combobox"]')!,list=root.querySelector<HTMLElement>('[role="listbox"]')!,options=[...list.querySelectorAll<HTMLButtonElement>('[role="option"]')];
 let disposed=false;
 trigger.disabled=root.hasAttribute('disabled');
 const outside=(event:PointerEvent)=>{if(!root.contains(event.target as Node))close(false);};
 const close=(restore=true)=>{list.hidden=true;trigger.setAttribute('aria-expanded','false');document.removeEventListener('pointerdown',outside);if(restore&&!disposed)trigger.focus();};
 const open=()=>{if(trigger.disabled||!options.length)return;list.hidden=false;trigger.setAttribute('aria-expanded','true');document.addEventListener('pointerdown',outside);(options.find(option=>option.getAttribute('aria-selected')==='true')??options[0])?.focus();};
 const setValue=(value:string)=>{const option=options.find(item=>item.dataset.value===value);if(!option)return;root.dataset.value=value;trigger.querySelector('span')!.textContent=option.textContent;options.forEach(item=>item.setAttribute('aria-selected',String(item===option)));};
 Object.defineProperty(root,'value',{configurable:true,get:()=>root.dataset.value,set:value=>setValue(String(value))});
 const select=(option:HTMLButtonElement)=>{setValue(option.dataset.value!);close();onChange?.(option.dataset.value!);root.dispatchEvent(new CustomEvent('change',{bubbles:true,detail:{value:option.dataset.value}}));};
 trigger.onclick=()=>list.hidden?open():close();
 trigger.onkeydown=event=>{if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();open();const selected=options.findIndex(option=>option.getAttribute('aria-selected')==='true'),index=nextListboxIndex(selected,event.key,options.length);options[index]?.focus();}else if(event.key==='Enter'||event.key===' '){event.preventDefault();list.hidden?open():close();}else if(event.key==='Escape'&&!list.hidden){event.preventDefault();close();}};
 list.onclick=event=>{const option=(event.target as HTMLElement).closest<HTMLButtonElement>('[role="option"]');if(option)select(option);};
 list.onkeydown=event=>{const index=options.indexOf(document.activeElement as HTMLButtonElement);if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();options[nextListboxIndex(index,event.key,options.length)]?.focus();}else if(event.key==='Enter'||event.key===' '){event.preventDefault();if(index>=0)select(options[index]);}else if(event.key==='Escape'){event.preventDefault();close();}else if(event.key==='Tab')close(false);};
 return()=>{if(disposed)return;disposed=true;document.removeEventListener('pointerdown',outside);trigger.onclick=null;trigger.onkeydown=null;list.onclick=null;list.onkeydown=null;};
}

export function bindDropdowns(container:ParentNode,onChange?:(root:HTMLElement,value:string)=>void){
 container.querySelectorAll<HTMLElement>('fluida-select').forEach(select=>{const optionElements=[...select.querySelectorAll('option')],options=optionElements.map(option=>({value:option.getAttribute('value')??option.textContent??'',label:option.textContent??''})),selected=optionElements.find(option=>option.hasAttribute('selected')),value=selected?.getAttribute('value')??selected?.textContent??options[0]?.value??'',attributes=[...select.attributes].map(attribute=>`${attribute.name}="${escapeHtml(attribute.value)}"`).join(' ');select.insertAdjacentHTML('afterend',dropdown(options,value,select.getAttribute('aria-label')??select.closest('label')?.childNodes[0]?.textContent?.trim()??'Choose option',attributes));select.remove();});
 const cleanups=[...container.querySelectorAll<HTMLElement>('[data-dropdown]')].map(root=>bindDropdown(root,value=>onChange?.(root,value)));
 let disposed=false;
 const cleanup=()=>{if(disposed)return;disposed=true;observer?.disconnect();cleanups.forEach(dispose=>dispose());};
 const observed=container instanceof Node&&container!==document;
 const observer=observed?new MutationObserver(()=>{if(container instanceof Element&&!container.isConnected)cleanup();}):undefined;
 if(observer)observer.observe(document,{childList:true,subtree:true});
 return cleanup;
}

export function requestText(label:string,initialValue=''){
 const previous=document.activeElement instanceof HTMLElement?document.activeElement:undefined,layer=document.createElement('div'),titleId=nextControlId('dialog-title'),inputId=nextControlId('dialog-input');
 layer.className='control-modal';layer.innerHTML=`<form class="control-dialog" role="dialog" aria-modal="true" aria-labelledby="${titleId}"><h2 id="${titleId}">${escapeHtml(label)}</h2><label for="${inputId}">${escapeHtml(label)}</label><input id="${inputId}"><div class="control-dialog-actions"><button type="button" class="control-secondary" data-cancel>Cancel</button><button type="submit" class="control-primary">Continue</button></div></form>`;document.body.append(layer);
 const form=layer.querySelector<HTMLFormElement>('form')!,input=layer.querySelector<HTMLInputElement>('input')!;input.value=initialValue;
 return new Promise<string|null>(resolve=>{let finished=false;const finish=(value:string|null)=>{if(finished)return;finished=true;document.removeEventListener('keydown',keydown,true);layer.remove();previous?.focus();resolve(value);},keydown=(event:KeyboardEvent)=>{if(event.key==='Escape'){event.preventDefault();finish(null);return;}if(event.key!=='Tab')return;const focusable=[...form.querySelectorAll<HTMLElement>('input,button:not([disabled])')],first=focusable[0],last=focusable.at(-1);if(!first||!last)return;if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}};document.addEventListener('keydown',keydown,true);layer.onpointerdown=event=>{if(event.target===layer)finish(null);};form.onsubmit=event=>{event.preventDefault();finish(input.value);};layer.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=()=>finish(null);input.focus();input.select();});
}
