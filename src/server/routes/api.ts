import {Router,type RequestHandler} from 'express';
import type {ArchiveRecord,CalendarEvent,ClockState,Settings,ShellSession} from '../../shared/types/domain.js';
import {requireObject} from '../middleware/http.js';
import {DataService} from '../services/dataService.js';
import {FileService} from '../services/fileService.js';
import {TerminalService} from '../services/terminalService.js';
import {SystemMonitorService} from '../services/systemMonitorService.js';
import {AppRegistryService} from '../services/appRegistryService.js';

const fail=(message:string,code='VALIDATION_ERROR',status=400)=>Object.assign(new Error(message),{status,code});
const owner=(request:{get(name:string):string|undefined})=>{const id=request.get('x-fluida-session');if(!id||!/^[-\w]{1,128}$/.test(id))throw fail('A valid x-fluida-session header is required','UNAUTHORIZED',401);return id;};
const route=(handler:RequestHandler):RequestHandler=>(request,response,next)=>Promise.resolve(handler(request,response,next)).catch(next);
const object=(value:unknown):value is Record<string,unknown>=>Boolean(value)&&typeof value==='object'&&!Array.isArray(value);

export function validateSettings(patch:unknown,current:Settings){
 if(!object(patch))throw fail('Invalid settings payload');
 const allowed=['theme','accent','wallpaper','fontScale','dockPosition','animation','reducedMotion','volume','brightness','filesView','notifications','accessibility','desktop','defaultApps'];
 if(Object.keys(patch).some(key=>!allowed.includes(key)))throw fail('Unknown setting');
 const nested:{[key:string]:string[]}={notifications:['enabled','sound','showPreviews','doNotDisturb'],accessibility:['highContrast','focusVisible','textScale'],desktop:['workspaceBehavior','shortcutLayout','dockAutoHide','clockFormat'],defaultApps:['text','images']};
 for(const [key,keys] of Object.entries(nested))if(patch[key]!==undefined&&(!object(patch[key])||Object.keys(patch[key]).some(child=>!keys.includes(child))))throw fail(`Invalid ${key} preferences`);
 const value={...current,...patch,notifications:{...current.notifications,...object(patch.notifications)?patch.notifications:{}},accessibility:{...current.accessibility,...object(patch.accessibility)?patch.accessibility:{}},desktop:{...current.desktop,...object(patch.desktop)?patch.desktop:{}},defaultApps:{...current.defaultApps,...object(patch.defaultApps)?patch.defaultApps:{}}} as Settings;
 if(!['light','dark','system'].includes(value.theme)||!['bottom','left','right'].includes(value.dockPosition)||!['full','reduced','none'].includes(value.animation)||!['grid','list'].includes(value.filesView)||!/^#[0-9a-f]{6}$/i.test(value.accent)||typeof value.wallpaper!=='string'||value.wallpaper.length>64||!Number.isFinite(value.fontScale)||value.fontScale<.8||value.fontScale>1.4||!Number.isFinite(value.volume)||value.volume<0||value.volume>100||!Number.isFinite(value.brightness)||value.brightness<10||value.brightness>100)throw fail('Invalid setting value');
 if(typeof value.reducedMotion!=='boolean')throw fail('Invalid reduced motion setting');
 if(!object(value.notifications)||Object.values(value.notifications).some(v=>typeof v!=='boolean'))throw fail('Invalid notification preferences');
 if(!object(value.accessibility)||typeof value.accessibility.highContrast!=='boolean'||typeof value.accessibility.focusVisible!=='boolean'||!Number.isFinite(value.accessibility.textScale)||value.accessibility.textScale<.8||value.accessibility.textScale>2)throw fail('Invalid accessibility preferences');
 if(!object(value.desktop)||!['restore','fresh'].includes(value.desktop.workspaceBehavior)||!['grid','list'].includes(value.desktop.shortcutLayout)||typeof value.desktop.dockAutoHide!=='boolean'||!['12h','24h'].includes(value.desktop.clockFormat))throw fail('Invalid desktop preferences');
 if(!object(value.defaultApps)||Object.values(value.defaultApps).some(v=>typeof v!=='string'||v.length>64))throw fail('Invalid default applications');
 return patch as Partial<Settings>;
}

export function apiRouter(files=new FileService(),data=new DataService(),services:{terminal?:TerminalService;monitor?:SystemMonitorService;registry?:AppRegistryService}={}){
 const terminal=services.terminal??new TerminalService(files),monitor=services.monitor??new SystemMonitorService(files,terminal);
 const builtins=data.apps.map(app=>({manifest:{...app,version:'1.0.0',entryPoint:`builtin:${app.id}`,capabilities:[]},source:{kind:'builtin'} as const,enabled:true,installedAt:new Date(0).toISOString()}));
 const registry=services.registry??new AppRegistryService(undefined,undefined,builtins),router=Router();
 router.get('/fs',route(async(req,res)=>{if(req.query.path!==undefined&&typeof req.query.path!=='string')throw fail('path must be a string');res.json({success:true,data:await files.list(typeof req.query.path==='string'?req.query.path:'')});}));
 router.get('/fs/search',route(async(req,res)=>{if(typeof req.query.q!=='string'||(req.query.path!==undefined&&typeof req.query.path!=='string'))throw fail('q and path must be strings');res.json({success:true,data:await files.search(String(req.query.path??''),req.query.q)});}));
 router.get('/fs/meta',route(async(req,res)=>{if(typeof req.query.path!=='string')throw fail('path must be a string');res.json({success:true,data:await files.metadata(req.query.path)});}));
 router.post('/fs/preview',requireObject(['path']),route(async(req,res)=>{if(req.body.maxBytes!==undefined&&typeof req.body.maxBytes!=='number')throw fail('maxBytes must be a number');res.json({success:true,data:await files.preview(req.body.path,req.body.maxBytes)});}));
 router.post('/fs/read',requireObject(['path']),route(async(req,res)=>res.json({success:true,data:{content:await files.read(req.body.path)}})));
 router.post('/fs/write',requireObject(['path','content']),route(async(req,res)=>{await files.write(req.body.path,req.body.content);res.json({success:true,data:null});}));
 router.post('/fs/mkdir',requireObject(['path']),route(async(req,res)=>{await files.mkdir(req.body.path);res.status(201).json({success:true,data:null});}));
 router.post('/fs/rename',requireObject(['from','to']),route(async(req,res)=>{await files.rename(req.body.from,req.body.to);res.json({success:true,data:null});}));
 router.post('/fs/copy',requireObject(['from','to']),route(async(req,res)=>{await files.copy(req.body.from,req.body.to);res.status(201).json({success:true,data:null});}));
 router.post('/fs/import',requireObject(['directory','name','content']),route(async(req,res)=>res.status(201).json({success:true,data:await files.importBase64(req.body.directory,req.body.name,req.body.content)})));
 router.get('/fs/download',route(async(req,res)=>{if(typeof req.query.path!=='string')throw fail('path must be a string');res.download(files.resolve(req.query.path),files.normalizeFilename(req.query.path.split('/').at(-1)||'download'));}));
 router.delete('/fs',requireObject(['path']),route(async(req,res)=>{if(req.body.recursive!==undefined&&typeof req.body.recursive!=='boolean')throw fail('recursive must be a boolean');await files.delete(req.body.path,req.body.recursive===true);res.json({success:true,data:null});}));

 router.get('/apps',route(async(_req,res)=>res.json({success:true,data:(await registry.list()).filter(app=>app.enabled)})));
 router.get('/apps/all',route(async(_req,res)=>res.json({success:true,data:await registry.list()})));
 router.post('/apps/install',requireObject(['packagePath']),route(async(req,res)=>res.status(201).json({success:true,data:await registry.install(req.body.packagePath)})));
 router.patch('/apps/:id',route(async(req,res)=>{if(typeof req.body?.enabled!=='boolean')throw fail('enabled must be a boolean');res.json({success:true,data:await registry.setEnabled(req.params.id,req.body.enabled)});}));
 router.delete('/apps/:id',route(async(req,res)=>{await registry.uninstall(req.params.id);res.json({success:true,data:null});}));

 router.get('/shell/session',route(async(_req,res)=>res.json({success:true,data:await data.session.read()})));
 router.put('/shell/session',route(async(req,res)=>{if(!object(req.body)||!Array.isArray(req.body.workspaces))throw fail('Invalid shell session');res.json({success:true,data:await data.saveSession(req.body as unknown as ShellSession)});}));
 router.get('/settings',route(async(_req,res)=>res.json({success:true,data:await data.settings.read()})));
 router.put('/settings',route(async(req,res)=>res.json({success:true,data:await data.saveSettings(validateSettings(req.body,await data.settings.read()))})));
 router.post('/settings/reset',route(async(_req,res)=>res.json({success:true,data:await data.resetSettings()})));

 router.post('/terminal/jobs',requireObject(['command','cwd']),route(async(req,res)=>res.status(201).json({success:true,data:await terminal.execute(owner(req),req.body)})));
 router.get('/terminal/jobs',route(async(req,res)=>res.json({success:true,data:terminal.list(owner(req))})));
 router.delete('/terminal/jobs/:id',route(async(req,res)=>res.json({success:true,data:terminal.terminate(owner(req),req.params.id)})));
 router.delete('/system/jobs/:id',route(async(req,res)=>res.json({success:true,data:terminal.terminate(owner(req),req.params.id)})));

 router.get('/app-state/clock',route(async(_req,res)=>res.json({success:true,data:await data.readAppState('clock')})));
 router.put('/app-state/clock',route(async(req,res)=>{if(!object(req.body)||!Array.isArray(req.body.worldClocks)||req.body.worldClocks.some((zone:unknown)=>typeof zone!=='string'||zone.length>128)||!Array.isArray(req.body.alarms)||req.body.alarms.some((alarm:unknown)=>!object(alarm)||typeof alarm.id!=='string'||typeof alarm.time!=='string'||typeof alarm.label!=='string'||typeof alarm.enabled!=='boolean')||typeof req.body.timerSeconds!=='number'||req.body.timerSeconds<0||typeof req.body.stopwatchSeconds!=='number'||req.body.stopwatchSeconds<0||typeof req.body.stopwatchRunning!=='boolean')throw fail('Invalid clock state');res.json({success:true,data:await data.saveAppState('clock',req.body as unknown as ClockState)});}));
 router.get('/app-state/calendar',route(async(_req,res)=>res.json({success:true,data:await data.readAppState('calendar')})));
 router.put('/app-state/calendar',route(async(req,res)=>{if(!object(req.body)||!Array.isArray(req.body.events)||req.body.events.some((event:unknown)=>!object(event)||typeof event.id!=='string'||typeof event.date!=='string'||!/^\d{4}-\d{2}-\d{2}$/.test(event.date)||typeof event.title!=='string'||!event.title.trim()||typeof event.details!=='string'))throw fail('Invalid calendar state');res.json({success:true,data:await data.saveAppState('calendar',{events:req.body.events as CalendarEvent[]})});}));
 router.get('/app-state/archive',route(async(_req,res)=>res.json({success:true,data:await data.readAppState('archive')})));
 router.put('/app-state/archive',route(async(req,res)=>{if(!object(req.body)||!Array.isArray(req.body.records)||req.body.records.some((record:unknown)=>!object(record)||typeof record.id!=='string'||typeof record.name!=='string'||!record.name.trim()||!Array.isArray(record.paths)||record.paths.some(item=>typeof item!=='string')||typeof record.createdAt!=='string'))throw fail('Invalid archive state');for(const record of req.body.records as ArchiveRecord[])for(const item of record.paths)files.resolve(item);res.json({success:true,data:await data.saveAppState('archive',{records:req.body.records as ArchiveRecord[]})});}));
 router.get('/photos',route(async(_req,res)=>{const rows=await files.search('','.',500);res.json({success:true,data:rows.filter(row=>row.mimeType.startsWith('image/'))});}));

 router.get('/notes',route(async(req,res)=>{const q=typeof req.query.q==='string'?req.query.q.toLowerCase():'';res.json({success:true,data:(await data.notes.read()).filter(note=>`${note.title} ${note.body}`.toLowerCase().includes(q))});}));
 router.post('/notes',requireObject(['title','body']),route(async(req,res)=>res.status(201).json({success:true,data:await data.saveNote(req.body)})));
 router.delete('/notes/:id',route(async(req,res)=>{await data.notes.write((await data.notes.read()).filter(note=>note.id!==req.params.id));res.json({success:true,data:null});}));
 router.get('/notifications',route(async(_req,res)=>res.json({success:true,data:await data.notifications.read()})));
 router.patch('/notifications/:id',route(async(req,res)=>{if(typeof req.body?.read!=='boolean')throw fail('read must be a boolean');const rows=await data.notifications.read(),row=rows.find(notification=>notification.id===req.params.id);if(!row)throw fail('Notification not found','NOT_FOUND',404);row.read=req.body.read;await data.notifications.write(rows);res.json({success:true,data:row});}));
 router.delete('/notifications/:id',route(async(req,res)=>{await data.notifications.write((await data.notifications.read()).filter(notification=>notification.id!==req.params.id));res.json({success:true,data:null});}));
 router.get('/system/resources',route(async(req,res)=>res.json({success:true,data:await monitor.snapshot(owner(req))})));
 router.get('/system',route(async(req,res)=>{const snapshot=await monitor.snapshot(owner(req));snapshot.apps=(await registry.list()).length;res.json({success:true,data:snapshot});}));
 return router;
}
