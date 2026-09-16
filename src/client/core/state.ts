import type {AppDefinition,Settings} from '../../shared/types/domain.js';
export interface WindowState{id:string;app:AppDefinition;element:HTMLElement;minimized:boolean} export const state:{apps:AppDefinition[];settings:Settings|null;windows:WindowState[];active?:string;z:number}={apps:[],settings:null,windows:[],z:10};
