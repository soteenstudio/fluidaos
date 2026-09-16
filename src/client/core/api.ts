export async function api<T=any>(url:string,options:RequestInit={}):Promise<T>{const response=await fetch(`/api${url}`,{...options,headers:{'Content-Type':'application/json',...options.headers}});const payload=await response.json();if(!response.ok)throw new Error(payload.error?.message||'Request failed');return payload.data??payload;}
export const body=(value:unknown)=>JSON.stringify(value);
