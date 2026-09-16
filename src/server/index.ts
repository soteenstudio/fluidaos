import express from 'express';
import cors from 'cors';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { apiRouter } from './routes/api.js';
import { errorHandler, notFound } from './middleware/http.js';

export function createApp() { const app=express(); app.use(cors()); app.use(express.json({limit:'8mb'})); app.use('/assets',express.static(path.resolve(process.cwd(),'dist'))); app.use('/styles',express.static(path.resolve(process.cwd(),'src/client/styles'))); app.use(express.static(path.resolve(process.cwd(),'src/public'))); app.use('/api',apiRouter()); app.use('/api',notFound); app.use(errorHandler); return app; }
const isMain=process.argv[1]&&fileURLToPath(import.meta.url)===path.resolve(process.argv[1]);
if(isMain) createApp().listen(Number(process.env.PORT)||3000,()=>console.log('FluidaOS ready at http://localhost:3000'));
