import express from 'express';
import cors from 'cors';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { apiRouter } from './routes/api.js';
import { authenticateSession, errorHandler, notFound } from './middleware/http.js';

export function createApp(options:{trustedOrigins?:string[];sessionSecret?:string}={}) { const app=express(),trusted=new Set(options.trustedOrigins??process.env.TRUSTED_ORIGINS?.split(',').map(value=>value.trim()).filter(Boolean)??[]); app.use(cors({credentials:true,origin:(origin,callback)=>callback(null,!origin||trusted.has(origin))})); app.use(express.json({limit:'8mb'})); app.use('/assets',express.static(path.resolve(process.cwd(),'dist'))); app.use('/styles',express.static(path.resolve(process.cwd(),'src/client/styles'))); app.use(express.static(path.resolve(process.cwd(),'src/public'))); app.use('/api',authenticateSession(options.sessionSecret)); app.use('/api',apiRouter()); app.use('/api',notFound); app.use(errorHandler); return app; }
const isMain=process.argv[1]&&fileURLToPath(import.meta.url)===path.resolve(process.argv[1]);
if(isMain){const port=Number(process.env.PORT)||3000,host=process.env.HOST||'127.0.0.1';createApp().listen(port,host,()=>console.log(`FluidaOS ready at http://${host}:${port}`));}
