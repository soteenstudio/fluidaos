import fs from 'node:fs/promises';
import {randomUUID} from 'node:crypto';
import path from 'node:path';
export class JsonRepository<T> {
 private queue:Promise<void>=Promise.resolve();
 constructor(private file:string,private fallback:T){}
 private serialized<R>(operation:()=>Promise<R>){const result=this.queue.then(operation,operation);this.queue=result.then(()=>undefined,()=>undefined);return result;}
 private async readUnlocked():Promise<T>{try{return JSON.parse(await fs.readFile(this.file,'utf8')) as T;}catch(error:any){if(error.code!=='ENOENT')throw error;const value=structuredClone(this.fallback);await this.writeUnlocked(value);return value;}}
 private async writeUnlocked(value:T):Promise<T>{await fs.mkdir(path.dirname(this.file),{recursive:true});const temporary=`${this.file}.${process.pid}.${randomUUID()}.tmp`;await fs.writeFile(temporary,JSON.stringify(value,null,2),'utf8');await fs.rename(temporary,this.file);return value;}
 read():Promise<T>{return this.serialized(()=>this.readUnlocked());}
 write(value:T):Promise<T>{return this.serialized(()=>this.writeUnlocked(value));}
 update(transform:(current:T)=>T|Promise<T>):Promise<T>{return this.serialized(async()=>this.writeUnlocked(await transform(await this.readUnlocked())));}
}
