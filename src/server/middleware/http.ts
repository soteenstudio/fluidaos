import type {ErrorRequestHandler,RequestHandler} from 'express';
export function requireObject(keys:string[]):RequestHandler{return(req,res,next)=>{if(!req.body||typeof req.body!=='object'||keys.some(key=>typeof req.body[key]!=='string'))return res.status(400).json({success:false,error:{code:'VALIDATION_ERROR',message:`Required string fields: ${keys.join(', ')}`}});next();};}
export const notFound:RequestHandler=(_req,res)=>{res.status(404).json({success:false,error:{code:'NOT_FOUND',message:'Endpoint not found'}});};
export const errorHandler:ErrorRequestHandler=(error,_req,res,_next)=>{const status=Number(error.status)||(error.code==='ENOENT'?404:500);res.status(status).json({success:false,error:{code:error.code||'INTERNAL_ERROR',message:status===500?'Unexpected server error':error.message}});};

