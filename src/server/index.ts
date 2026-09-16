import express from 'express';
import cors from 'cors';
import path from 'path';
import { fsManager } from './fsManager.js';
import { appManager } from './appManager.js';

const app = express();
const PORT = 3000;

app.use(cors());
app.use(express.json());
app.use(express.static(path.resolve(process.cwd(), 'src/public')));

// --- API File System ---
app.get('/api/fs', async (req, res) => {
    try {
        const dirPath = (req.query.path as string) || '';
        const files = await fsManager.listDir(dirPath);
        res.json({ success: true, data: files });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

app.post('/api/fs/read', async (req, res) => {
    try {
        const { path: filePath } = req.body;
        const content = await fsManager.readFile(filePath);
        res.json({ success: true, content });
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

app.post('/api/fs/write', async (req, res) => {
    try {
        const { path: filePath, content } = req.body;
        const result = await fsManager.writeFile(filePath, content);
        res.json(result);
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

app.delete('/api/fs', async (req, res) => {
    try {
        const { path: targetPath } = req.body;
        const result = await fsManager.deleteItem(targetPath);
        res.json(result);
    } catch (error: any) {
        res.status(500).json({ success: false, error: error.message });
    }
});

// --- API Aplikasi ---
app.get('/api/apps', (req, res) => {
    res.json({ success: true, data: appManager.getApps() });
});

app.listen(PORT, () => {
    console.log(`[OS Core] Berhasil nyala! Akses OS lu di: http://localhost:${PORT}`);
});