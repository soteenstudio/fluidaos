import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import express from 'express';
import type {AddressInfo} from 'node:net';
import {apiRouter} from '../routes/api.js';
import {createApp} from '../index.js';
import {authenticateSession,errorHandler} from '../middleware/http.js';
import {AppRegistryService} from '../services/appRegistryService.js';
import {DataService} from '../services/dataService.js';
import {FileService} from '../services/fileService.js';
import {SystemMonitorService} from '../services/systemMonitorService.js';
import {TerminalService} from '../services/terminalService.js';

async function withApi(run:(origin:string)=>Promise<void>){const root=await fs.mkdtemp(path.join(os.tmpdir(),'fluida-api-')),files=new FileService(path.join(root,'files')),data=new DataService(path.join(root,'data')),terminal=new TerminalService(files),monitor=new SystemMonitorService(files,terminal),registry=new AppRegistryService(path.join(root,'apps'),path.join(root,'data'));const app=express();app.use(express.json());app.use(authenticateSession('test-session-secret'));app.use(apiRouter(files,data,{terminal,monitor,registry}));app.use(errorHandler);const server=app.listen(0,'127.0.0.1');await new Promise<void>((resolve,reject)=>{server.once('listening',resolve);server.once('error',reject);});try{const address=server.address() as AddressInfo;await run(`http://127.0.0.1:${address.port}`);}finally{await new Promise<void>((resolve,reject)=>server.close(error=>error?reject(error):resolve()));}}

test('settings API validates strict field types before persistence',()=>withApi(async origin=>{const invalid=await fetch(`${origin}/settings`,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({volume:'80'})});assert.equal(invalid.status,400);assert.equal((await invalid.json() as any).error.message,'Invalid request');const valid=await fetch(`${origin}/settings`,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({volume:80,reducedMotion:true,notifications:{sound:false}})});assert.equal(valid.status,200);const payload=await valid.json() as any;assert.equal(payload.data.volume,80);assert.equal(payload.data.reducedMotion,true);assert.equal(payload.data.notifications.sound,false);}));

test('signed sessions prevent headers from selecting another owner jobs',()=>withApi(async origin=>{const created=await fetch(`${origin}/terminal/jobs`,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({command:'touch owner-file',cwd:''})}),cookie=created.headers.get('set-cookie')?.split(';',1)[0];assert.equal(created.status,201);assert.ok(cookie);const own=await fetch(`${origin}/system`,{headers:{cookie}});assert.equal(((await own.json()) as any).data.jobs.length,1);const other=await fetch(`${origin}/system`,{headers:{'x-fluida-session':'forged-owner'}});assert.equal(((await other.json()) as any).data.jobs.length,0);assert.match(other.headers.get('set-cookie')??'',/HttpOnly/);}));

test('CORS exposes API responses only to configured origins',async()=>{const app=createApp({trustedOrigins:['https://trusted.example'],sessionSecret:'test-session-secret'}),server=app.listen(0,'127.0.0.1');await new Promise<void>((resolve,reject)=>{server.once('listening',resolve);server.once('error',reject);});try{const address=server.address() as AddressInfo,origin=`http://127.0.0.1:${address.port}`,headers={'access-control-request-method':'PUT'};const trusted=await fetch(`${origin}/api/settings`,{method:'OPTIONS',headers:{...headers,origin:'https://trusted.example'}}),untrusted=await fetch(`${origin}/api/settings`,{method:'OPTIONS',headers:{...headers,origin:'https://untrusted.example'}});assert.equal(trusted.headers.get('access-control-allow-origin'),'https://trusted.example');assert.equal(untrusted.headers.get('access-control-allow-origin'),null);}finally{await new Promise<void>((resolve,reject)=>server.close(error=>error?reject(error):resolve()));}});
