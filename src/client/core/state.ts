import type {AppDefinition,Settings,ShellSession,WindowState} from '../../shared/types/domain.js';
export interface RuntimeWindow { id:string;app:AppDefinition;element:HTMLElement;model:WindowState }
export const state:{apps:AppDefinition[];settings:Settings|null;session:ShellSession|null;runtime:Map<string,RuntimeWindow>;saveTimer?:number}={apps:[],settings:null,session:null,runtime:new Map()};
export function activeWorkspace(){const session=state.session;if(!session)throw new Error('Shell session is not loaded');const workspace=session.workspaces.find(item=>item.id===session.activeWorkspaceId);if(!workspace)throw new Error('Active workspace is missing');return workspace;}
export function activeRuntimeWindows(){const ids=new Set(activeWorkspace().windows.map(window=>window.id));return [...state.runtime.values()].filter(window=>ids.has(window.id));}
