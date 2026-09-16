import fs from 'fs/promises';
import path from 'path';

const STORAGE_ROOT = path.resolve(process.cwd(), 'os_storage');

// Inisialisasi folder root penyimpanan OS jika belum ada
async function ensureStorage() {
    try {
        await fs.mkdir(STORAGE_ROOT, { recursive: true });
    } catch (error) {
        console.error('Gagal membuat storage root:', error);
    }
}

ensureStorage();

export const fsManager = {
    async listDir(dirPath: string = '') {
        const targetPath = path.join(STORAGE_ROOT, dirPath);
        const entries = await fs.readdir(targetPath, { withFileTypes: true });
        
        return entries.map(entry => ({
            name: entry.name,
            isDirectory: entry.isDirectory(),
            path: path.join(dirPath, entry.name)
        }));
    },

    async readFile(filePath: string) {
        const targetPath = path.join(STORAGE_ROOT, filePath);
        return await fs.readFile(targetPath, 'utf-8');
    },

    async writeFile(filePath: string, content: string) {
        const targetPath = path.join(STORAGE_ROOT, filePath);
        await fs.mkdir(path.dirname(targetPath), { recursive: true });
        await fs.writeFile(targetPath, content, 'utf-8');
        return { success: true, message: 'File berhasil disimpan' };
    },

    async deleteItem(targetPath: string) {
        const fullPath = path.join(STORAGE_ROOT, targetPath);
        const stats = await fs.stat(fullPath);
        if (stats.isDirectory()) {
            await fs.rm(fullPath, { recursive: true, force: true });
        } else {
            await fs.unlink(fullPath);
        }
        return { success: true, message: 'Item berhasil dihapus' };
    }
};