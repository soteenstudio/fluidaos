import fs from 'node:fs/promises';
import path from 'node:path';
export class JsonRepository<T> { constructor(private file:string,private fallback:T){} async read():Promise<T>{try{return JSON.parse(await fs.readFile(this.file,'utf8')) as T;}catch(error:any){if(error.code!=='ENOENT')throw error;await this.write(this.fallback);return structuredClone(this.fallback);}} async write(value:T):Promise<T>{await fs.mkdir(path.dirname(this.file),{recursive:true});const temporary=`${this.file}.${process.pid}.tmp`;await fs.writeFile(temporary,JSON.stringify(value,null,2),'utf8');await fs.rename(temporary,this.file);return value;} }

