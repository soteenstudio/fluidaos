import fs from 'node:fs/promises';
import path from 'node:path';
import {randomUUID} from 'node:crypto';
import type {TerminalJob,TerminalJobState,TerminalResult} from '../../shared/types/domain.js';
import {terminalCommandNames} from '../../shared/types/domain.js';
import {FileService} from './fileService.js';

interface RunningJob { job:TerminalJob }
const commandError=(message:string)=>Object.assign(new Error(message),{status:400,code:'TERMINAL_COMMAND_REJECTED'});
const usage=(name:string,value:string)=>commandError(`${name}: usage: ${value}`);

export class TerminalService {
 private jobs=new Map<string,RunningJob>();
 constructor(private files=new FileService(),_scripts:Record<string,never>={},private timeoutMs=5000,private outputLimit=64*1024){}
 list(ownerId?:string){return [...this.jobs.values()].map(item=>item.job).filter(job=>!ownerId||job.ownerId===ownerId);}
 private tokenize(command:string){const tokens:string[]=[];let value='',quote='';for(let index=0;index<command.length;index++){const char=command[index];if(quote){if(char===quote)quote='';else if(char==='\\'&&index+1<command.length)value+=command[++index];else value+=char;}else if(char==='"'||char==="'")quote=char;else if(/\s/.test(char)){if(value){tokens.push(value);value='';}}else if(char==='\\'&&index+1<command.length)value+=command[++index];else value+=char;}if(quote)throw commandError('Unterminated quote');if(value)tokens.push(value);return tokens;}
 private target(cwd:string,input=''){if(path.posix.isAbsolute(input)||path.win32.isAbsolute(input))throw commandError('Absolute paths are not allowed');const candidate=path.posix.normalize(path.posix.join(cwd,input));if(candidate==='..'||candidate.startsWith('../'))throw commandError('Path escapes OS storage');const joined=candidate==='.'?'':candidate;this.files.resolve(joined);return joined;}
 private requireCount(name:string,tokens:string[],min:number,max=min){if(tokens.length<min||tokens.length>max)throw usage(name,`${min===max?min:`${min}-${max}`} argument${max===1?'':'s'}`);}
 private bounded(value:string){if(Buffer.byteLength(value)>this.outputLimit)throw commandError('Command output limit exceeded');return value;}
 private async lines(file:string,count:number,tail=false){if(!Number.isInteger(count)||count<0||count>10000)throw commandError('Line count must be between 0 and 10000');const rows=(await this.files.read(file)).split(/\r?\n/);return(tail?rows.slice(-count):rows.slice(0,count)).join('\n');}
 async execute(ownerId:string,input:{command:string;cwd:string}):Promise<TerminalResult>{
  if(!ownerId||typeof input.command!=='string'||typeof input.cwd!=='string'||input.command.length>4096)throw commandError('Invalid terminal request');
  const started=Date.now(),tokens=this.tokenize(input.command),name=tokens.shift();if(!name)throw commandError('Command is required');if(!(terminalCommandNames as readonly string[]).includes(name))throw commandError(`Command not allowed: ${name}`);
  const cwd=this.target('',input.cwd),job:TerminalJob={id:randomUUID(),ownerId,command:input.command,cwd,state:{kind:'running'},startedAt:new Date().toISOString()};this.jobs.set(job.id,{job});
  try{let stdout='',nextCwd=cwd;const at=(value='')=>this.target(cwd,value);
   switch(name){
    case'help':this.requireCount(name,tokens,0);stdout=`Restricted commands:\n${terminalCommandNames.join('  ')}\nAll paths are contained in FluidaOS storage.`;break;
    case'pwd':this.requireCount(name,tokens,0);stdout=`/${cwd}`;break;
    case'cd':this.requireCount(name,tokens,0,1);{const target=at(tokens[0]??'');if(!(await fs.stat(this.files.resolve(target))).isDirectory())throw commandError('Not a directory');nextCwd=target;}break;
    case'ls':this.requireCount(name,tokens,0,1);stdout=(await this.files.list(at(tokens[0]))).map(entry=>`${entry.isDirectory?'d':'-'} ${entry.name}`).join('\n');break;
    case'tree':this.requireCount(name,tokens,0,1);{const root=at(tokens[0]),rows:string[]=[root||'.'];const walk=async(dir:string,prefix:string):Promise<void>=>{const entries=await this.files.list(dir);for(let i=0;i<entries.length;i++){const entry=entries[i],last=i===entries.length-1;rows.push(`${prefix}${last?'└──':'├──'} ${entry.name}`);if(entry.isDirectory)await walk(entry.path,`${prefix}${last?'    ':'│   '}`);}};await walk(root,'');stdout=rows.join('\n');}break;
    case'cat':this.requireCount(name,tokens,1);stdout=await this.files.read(at(tokens[0]));break;
    case'head':case'tail':this.requireCount(name,tokens,1,2);stdout=await this.lines(at(tokens[0]),tokens[1]===undefined?10:Number(tokens[1]),name==='tail');break;
    case'mkdir':this.requireCount(name,tokens,1);await this.files.mkdir(at(tokens[0]));break;
    case'rmdir':this.requireCount(name,tokens,1,2);if(tokens[1]!==undefined&&tokens[1]!=='--recursive')throw usage(name,'PATH [--recursive]');await this.files.delete(at(tokens[0]),tokens[1]==='--recursive');break;
    case'touch':this.requireCount(name,tokens,1);await this.files.touch(at(tokens[0]));break;
    case'write':case'append':this.requireCount(name,tokens,2,Number.MAX_SAFE_INTEGER);{const file=at(tokens.shift()),content=tokens.join(' ');if(name==='write')await this.files.write(file,content);else await this.files.append(file,content);}break;
    case'cp':this.requireCount(name,tokens,2);await this.files.copy(at(tokens[0]),at(tokens[1]));break;
    case'mv':this.requireCount(name,tokens,2);await this.files.rename(at(tokens[0]),at(tokens[1]));break;
    case'rm':this.requireCount(name,tokens,1);{const file=at(tokens[0]);if((await this.files.metadata(file)).isDirectory)throw commandError('rm only removes files; use rmdir for directories');await this.files.delete(file);}break;
    case'find':this.requireCount(name,tokens,1,2);stdout=(await this.files.search(tokens[1]?at(tokens[0]):cwd,tokens[1]??tokens[0])).map(entry=>entry.path).join('\n');break;
    case'stat':this.requireCount(name,tokens,1);{const entry=await this.files.metadata(at(tokens[0]));stdout=[`Path: ${entry.path}`,`Type: ${entry.isDirectory?'directory':'file'}`,`Size: ${entry.size}`,`Modified: ${entry.modifiedAt}`,`MIME: ${entry.mimeType}`].join('\n');}break;
    case'date':this.requireCount(name,tokens,0);stdout=new Date().toISOString();break;
    case'clear':this.requireCount(name,tokens,0);stdout='';break;
   }
   if(Date.now()-started>this.timeoutMs) return this.finish(job,{kind:'timed-out'},'','',null,cwd,started);
   return this.finish(job,{kind:'completed',exitCode:0},this.bounded(stdout),'',0,nextCwd,started);
  }catch(error){const state:TerminalJobState={kind:'failed',exitCode:null,reason:(error as Error).message};this.finish(job,state,'',(error as Error).message,null,cwd,started);throw error;}
 }
 private finish(job:TerminalJob,state:TerminalJobState,stdout:string,stderr:string,exitCode:number|null,cwd:string,started:number):TerminalResult{job.state=state;job.finishedAt=new Date().toISOString();return{jobId:job.id,stdout,stderr,exitCode,duration:Date.now()-started,cwd,state};}
 terminate(ownerId:string,id:string){const running=this.jobs.get(id);if(!running)throw Object.assign(new Error('Terminal job not found'),{status:404,code:'NOT_FOUND'});if(running.job.ownerId!==ownerId)throw Object.assign(new Error('Terminal job belongs to another session'),{status:403,code:'FORBIDDEN'});if(running.job.state.kind!=='running')throw commandError('Terminal job is not running');running.job.state={kind:'cancelled'};running.job.finishedAt=new Date().toISOString();return running.job;}
}
