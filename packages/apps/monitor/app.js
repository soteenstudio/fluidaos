const manifest={"id":"monitor","name":"Monitor","icon":"⌁","description":"Inspect this session"};
const root=document.querySelector('#app');root.innerHTML='<h1>'+manifest.icon+' '+manifest.name+'</h1><p>'+manifest.description+'</p><small>Running in an isolated FluidaOS package.</small>';
window.addEventListener('message',event=>{if(event.source!==parent||event.data?.fluida!==true||event.data?.type!=='init'||event.data?.appId!==manifest.id)return;document.documentElement.dataset.ready='true'});
