export interface Settings { theme: 'light' | 'dark' | 'system'; accent: string; wallpaper: string; fontScale: number; dockPosition: 'bottom' | 'left'; animation: 'full' | 'reduced' | 'none'; reducedMotion: boolean }
export interface Note { id: string; title: string; body: string; folder: string; updatedAt: string }
export interface Notification { id: string; title: string; message: string; read: boolean; createdAt: string }
export interface AppDefinition { id: string; name: string; icon: string; description: string }

