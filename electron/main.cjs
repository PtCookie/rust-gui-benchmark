const { app, BrowserWindow, ipcMain } = require("electron");
const path = require("path");
const fs = require("fs");
const addon = require("./addon.node");

const scenario = process.env.BENCH_SCENARIO || "commits";
const OUT = process.env.BENCH_OUT || "/tmp/bench-electron.json";
const REPO = process.env.BENCH_REPO;
const DB = process.env.BENCH_DB;
let coreLoadMs = 0;

app.commandLine.appendSwitch("no-sandbox");
app.commandLine.appendSwitch("disable-gpu"); // no GPU in the test environment: software compositing
app.commandLine.appendSwitch("disable-dev-shm-usage");

ipcMain.handle("open", () => {
  const info = addon.open(scenario, REPO, DB);
  coreLoadMs = info.coreLoadMs;
  return { scenario: info.scenario, total: info.total, max_width: info.maxWidth };
});
ipcMain.handle("commits_chunk", (_e, a) => addon.commitsChunk(a.offset, a.count));
ipcMain.handle("grid_page", (_e, a) => addon.gridPage(a.offset, a.count));
ipcMain.handle("mark", (_e, a) => { fs.writeFileSync(OUT + ".mark_" + a.name, ""); });
ipcMain.handle("ready", () => { fs.writeFileSync(OUT + ".ready", ""); });
ipcMain.handle("report", (_e, r) => {
  r.framework = "electron";
  r.core_load_ms = coreLoadMs;
  fs.writeFileSync(OUT, JSON.stringify(r, null, 2));
  setTimeout(() => app.quit(), 50);
});

app.whenReady().then(() => {
  const win = new BrowserWindow({
    width: 1200, height: 780, useContentSize: true,
    webPreferences: { preload: path.join(__dirname, "preload.cjs"), contextIsolation: true, sandbox: false },
  });
  win.loadFile(path.join(__dirname, "..", "web", "dist", "index.html"));
});
