export interface DropdownOption { value:string;label:string }

const escapeHtml=(value:string)=>value.replace(/[&<>"']/g,character=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[character]!));

export function dropdown(options:DropdownOption[],value:string,label:string,attributes=''){
 const selected=options.find(option=>option.value===value)??options[0];
 return `<div class="custom-dropdown" data-dropdown data-value="${escapeHtml(selected?.value??'')}" ${attributes}><button type="button" class="dropdown-trigger" role="combobox" aria-label="${escapeHtml(label)}" aria-haspopup="listbox" aria-expanded="false"><span>${escapeHtml(selected?.label??'')}</span><i aria-hidden="true">⌄</i></button><div class="dropdown-list" role="listbox" aria-label="${escapeHtml(label)}" hidden>${options.map(option=>`<button type="button" role="option" data-value="${escapeHtml(option.value)}" aria-selected="${option.value===selected?.value}">${escapeHtml(option.label)}</button>`).join('')}</div></div>`;
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
 const close=(restore=true)=>{list.hidden=true;trigger.setAttribute('aria-expanded','false');document.removeEventListener('pointerdown',outside);if(restore)trigger.focus();};
 const open=()=>{list.hidden=false;trigger.setAttribute('aria-expanded','true');document.addEventListener('pointerdown',outside);(options.find(option=>option.getAttribute('aria-selected')==='true')??options[0])?.focus();};
 const outside=(event:PointerEvent)=>{if(!root.contains(event.target as Node))close(false);};
 const setValue=(value:string)=>{const option=options.find(item=>item.dataset.value===value);if(!option)return;root.dataset.value=value;trigger.querySelector('span')!.textContent=option.textContent;options.forEach(item=>item.setAttribute('aria-selected',String(item===option)));};
 Object.defineProperty(root,'value',{configurable:true,get:()=>root.dataset.value,set:value=>setValue(String(value))});
 const select=(option:HTMLButtonElement)=>{setValue(option.dataset.value!);close();onChange?.(option.dataset.value!);root.dispatchEvent(new CustomEvent('change',{bubbles:true,detail:{value:option.dataset.value}}));};
 trigger.onclick=()=>list.hidden?open():close();
 trigger.onkeydown=event=>{if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();open();const selected=options.findIndex(option=>option.getAttribute('aria-selected')==='true'),index=nextListboxIndex(selected,event.key,options.length);options[index]?.focus();}else if(event.key==='Escape'&&!list.hidden){event.preventDefault();close();}};
 list.onclick=event=>{const option=(event.target as HTMLElement).closest<HTMLButtonElement>('[role="option"]');if(option)select(option);};
 list.onkeydown=event=>{const index=options.indexOf(document.activeElement as HTMLButtonElement);if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();options[nextListboxIndex(index,event.key,options.length)]?.focus();}else if(event.key==='Enter'||event.key===' '){event.preventDefault();if(index>=0)select(options[index]);}else if(event.key==='Escape'){event.preventDefault();close();}else if(event.key==='Tab')close(false);};
 return()=>{document.removeEventListener('pointerdown',outside);};
}

export function bindDropdowns(container:ParentNode,onChange?:(root:HTMLElement,value:string)=>void){
 container.querySelectorAll<HTMLElement>('fluida-select').forEach(select=>{const optionElements=[...select.querySelectorAll('option')],options=optionElements.map(option=>({value:option.getAttribute('value')??option.textContent??'',label:option.textContent??''})),selected=optionElements.find(option=>option.hasAttribute('selected')),value=selected?.getAttribute('value')??selected?.textContent??options[0]?.value??'',attributes=[...select.attributes].map(attribute=>`${attribute.name}="${escapeHtml(attribute.value)}"`).join(' ');select.insertAdjacentHTML('afterend',dropdown(options,value,select.getAttribute('aria-label')??select.closest('label')?.childNodes[0]?.textContent?.trim()??'Choose option',attributes));select.remove();});
 container.querySelectorAll<HTMLElement>('[data-dropdown]').forEach(root=>bindDropdown(root,value=>onChange?.(root,value)));
}

export function requestText(label:string,initialValue=''){
 const previous=document.activeElement instanceof HTMLElement?document.activeElement:undefined,layer=document.createElement('div');
 layer.className='control-modal';layer.innerHTML=`<form class="control-dialog" role="dialog" aria-modal="true" aria-labelledby="control-dialog-title"><h2 id="control-dialog-title">${escapeHtml(label)}</h2><input aria-label="${escapeHtml(label)}"><div><button type="button" data-cancel>Cancel</button><button type="submit">Continue</button></div></form>`;document.body.append(layer);
 const form=layer.querySelector<HTMLFormElement>('form')!,input=layer.querySelector<HTMLInputElement>('input')!;input.value=initialValue;
 return new Promise<string|null>(resolve=>{const finish=(value:string|null)=>{document.removeEventListener('keydown',keydown,true);layer.remove();previous?.focus();resolve(value);},keydown=(event:KeyboardEvent)=>{if(event.key==='Escape'){event.preventDefault();finish(null);}};document.addEventListener('keydown',keydown,true);layer.onpointerdown=event=>{if(event.target===layer)finish(null);};form.onsubmit=event=>{event.preventDefault();finish(input.value);};layer.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=()=>finish(null);input.focus();input.select();});
}
