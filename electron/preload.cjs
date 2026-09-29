const { contextBridge, ipcRenderer } = require("electron");
const api = {};
for (const c of ["open", "commits_chunk", "grid_page", "ready", "report", "mark"]) api[c] = (args) => ipcRenderer.invoke(c, args);
contextBridge.exposeInMainWorld("benchApi", api);
