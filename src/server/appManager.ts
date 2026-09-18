export interface AppDefinition {
    id: string;
    name: string;
    icon: string;
    executable: string; // Bisa berupa script atau perintah sistem
}

let installedApps: AppDefinition[] = [
    { id: 'text-editor', name: 'Text Editor', icon: '📝', executable: 'internal' },
    { id: 'file-explorer', name: 'File Manager', icon: '📁', executable: 'internal' },
    { id: 'terminal', name: 'Terminal', icon: '💻', executable: 'internal' }
];

export const appManager = {
    getApps() {
        return installedApps;
    },
    installApp(app: AppDefinition) {
        installedApps.push(app);
        return { success: true, apps: installedApps };
    }
};