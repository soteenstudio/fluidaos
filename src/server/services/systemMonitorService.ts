import type {ResourceSnapshot} from '../../shared/types/domain.js';
import {FileService} from './fileService.js';
import {TerminalService} from './terminalService.js';
export class SystemMonitorService { constructor(private files:FileService,private terminal:TerminalService){} async snapshot():Promise<ResourceSnapshot>{const memory=process.memoryUsage(),cpu=process.cpuUsage();return{timestamp:new Date().toISOString(),uptime:process.uptime(),platform:process.platform,cpuUsage:{user:cpu.user,system:cpu.system},memory:{rss:memory.rss,heapUsed:memory.heapUsed,heapTotal:memory.heapTotal},processId:process.pid,storage:await this.files.usage(),jobs:this.terminal.list().map(job=>({id:job.id,ownerId:job.ownerId,label:job.command,state:job.state,startedAt:job.startedAt}))};}}
