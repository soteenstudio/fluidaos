export interface WindowBounds { x:number; y:number; width:number; height:number }
export type WindowSnap={kind:'none'}|{kind:'half';position:'left'|'right'|'top'|'bottom'}|{kind:'quadrant';position:'top-left'|'top-right'|'bottom-left'|'bottom-right'};
export type WindowMode={kind:'normal'}|{kind:'maximized';restoreBounds:WindowBounds}|{kind:'snapped';snap:Exclude<WindowSnap,{kind:'none'}>;restoreBounds:WindowBounds};
export interface WindowState { id:string;appId:string;title:string;bounds:WindowBounds;mode:WindowMode;minimized:boolean;zIndex:number;appState?:Record<string,unknown> }
export interface DesktopShortcutPosition { x:number;y:number }
export interface Workspace { id:string;name:string;windows:WindowState[];activeWindowId?:string;zIndexSequence:number;layout:'freeform'|'tiled';shortcutPositions?:Record<string,DesktopShortcutPosition> }
export interface ShellSession { id:string;workspaces:Workspace[];activeWorkspaceId:string;pinnedApplications:string[];updatedAt:string }
export const terminalCommandNames=['help','pwd','cd','ls','tree','cat','head','tail','mkdir','rmdir','touch','write','append','cp','mv','rm','find','stat','date','clear'] as const;
export type TerminalCommandName=typeof terminalCommandNames[number];
export interface TerminalCommand { command:string;cwd:string }
export type TerminalJobState={kind:'running'}|{kind:'completed';exitCode:number}|{kind:'failed';exitCode:number|null;reason:string}|{kind:'cancelled'}|{kind:'timed-out'};
export interface TerminalJob { id:string;ownerId:string;command:string;cwd:string;state:TerminalJobState;startedAt:string;finishedAt?:string }
export interface TerminalResult { jobId:string;stdout:string;stderr:string;exitCode:number|null;duration:number;cwd:string;state:TerminalJobState }
export interface FileEntry { name:string;path:string;isDirectory:boolean;size:number;modifiedAt:string;mimeType:string }
export interface FileSearchResult extends FileEntry { parent:string }
export interface FilePreview { path:string;content:string;truncated:boolean;size:number;mimeType:string }
export interface ResourceSnapshot { timestamp:string;uptime:number;platform:string;cpuUsage:{user:number;system:number};memory:{rss:number;heapUsed:number;heapTotal:number};processId:number;storage:{bytes:number;files:number};jobs:ManagedTask[];apps?:number }
export interface ManagedTask { id:string;ownerId:string;label:string;state:TerminalJobState;startedAt:string }
export type ApplicationSource={kind:'builtin'}|{kind:'local';packagePath:string};
export type AppCapability='files:read'|'files:write'|'terminal'|'notifications'|'system:read';
export interface AppManifest { id:string;name:string;icon:string;description:string;version:string;entryPoint:string;capabilities:AppCapability[] }
export interface InstalledApp { manifest:AppManifest;source:ApplicationSource;enabled:boolean;installedAt:string }
export type AppInstallationResult={kind:'installed';application:InstalledApp}|{kind:'rejected';reason:string};
export interface AppInstallRequest { packagePath:string }
export interface NotificationPreferences { enabled:boolean;sound:boolean;showPreviews:boolean;doNotDisturb:boolean }
export interface AccessibilityPreferences { highContrast:boolean;focusVisible:boolean;textScale:number }
export interface DesktopPreferences { workspaceBehavior:'restore'|'fresh';shortcutLayout:'grid'|'list';dockAutoHide:boolean;clockFormat:'12h'|'24h' }
export interface DefaultApplications { text:string;images:string }
export interface QuickTogglePreferences { theme:'light'|'dark'|'system';reducedMotion:boolean;volume:number;brightness:number;dockPosition:'bottom'|'left'|'right' }
export interface Settings extends QuickTogglePreferences { accent:string;wallpaper:string;fontScale:number;animation:'full'|'reduced'|'none';filesView:'grid'|'list';notifications:NotificationPreferences;accessibility:AccessibilityPreferences;desktop:DesktopPreferences;defaultApps:DefaultApplications }
export interface Note { id:string;title:string;body:string;folder:string;updatedAt:string }
export interface Notification { id:string;title:string;message:string;read:boolean;createdAt:string;source?:string }
export interface AppDefinition { id:string;name:string;icon:string;description:string }
export interface ClockState { worldClocks:string[];alarms:{id:string;time:string;label:string;enabled:boolean}[];timerSeconds:number;stopwatchSeconds:number;stopwatchRunning:boolean }
export interface CalendarEvent { id:string;date:string;title:string;details:string }
export interface ArchiveRecord { id:string;name:string;paths:string[];createdAt:string }
export interface AppStateMap { clock:ClockState;calendar:{events:CalendarEvent[]};archive:{records:ArchiveRecord[]} }
