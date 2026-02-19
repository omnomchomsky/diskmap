const path = require('path');
const fs = require('fs');
const { app, BrowserWindow, ipcMain, dialog } = require('electron');
const { Session, UnixFsAdapter, WindowsFsAdapter } = require('../dm_core');

function getFilesystemStats(targetPath) {
  let canonical = targetPath;
  try {
    canonical = fs.realpathSync(targetPath);
  } catch (_error) {
    canonical = targetPath;
  }

  if (typeof fs.statfsSync !== 'function') {
    return null;
  }

  try {
    const stats = fs.statfsSync(canonical);
    const total = stats.bsize * stats.blocks;
    const free = stats.bsize * stats.bavail;
    return [total, free];
  } catch (_error) {
    return null;
  }
}

function serializeTree(tree, rootId) {
  const nodes = [];
  const stack = [[rootId, 0]];

  while (stack.length > 0) {
    const [nodeId, depth] = stack.pop();
    const node = tree.node(nodeId);

    nodes.push({
      id: nodeId,
      parent: node.parentId(),
      name: node.nameValue(),
      size: node.totalBytes(),
      own_size: node.ownBytes(),
      children: node.childrenValue().slice(),
      top_files: node
        .topFiles()
        .toSortedVecDesc()
        .map((file) => ({ name: file.name, size: file.size })),
      depth,
    });

    const children = node.childrenValue();
    for (let i = 0; i < children.length; i += 1) {
      stack.push([children[i], depth + 1]);
    }
  }

  return nodes;
}

function serializeTreeLimited(tree, rootId, maxDepth) {
  const nodes = [];
  const stack = [[rootId, 0]];

  while (stack.length > 0) {
    const [nodeId, depth] = stack.pop();
    const node = tree.node(nodeId);
    const children = depth < maxDepth ? node.childrenValue().slice() : [];

    nodes.push({
      id: nodeId,
      parent: node.parentId(),
      name: node.nameValue(),
      size: node.totalBytes(),
      own_size: node.ownBytes(),
      children,
      top_files: node
        .topFiles()
        .toSortedVecDesc()
        .map((file) => ({ name: file.name, size: file.size })),
      depth,
    });

    if (depth < maxDepth) {
      const childIds = node.childrenValue();
      for (let i = 0; i < childIds.length; i += 1) {
        stack.push([childIds[i], depth + 1]);
      }
    }
  }

  return nodes;
}

async function scanDirectory(pathValue) {
  const start = Date.now();
  const fsAdapter = new UnixFsAdapter();
  const session = new Session(pathValue, 10);
  const fsStats = getFilesystemStats(pathValue);

  await session.run(fsAdapter);

  const durationMs = Date.now() - start;
  const rootId = session.tree.rootId();
  const totalSize = session.tree.node(rootId).totalBytes();

  return {
    tree: serializeTree(session.tree, rootId),
    root_id: rootId,
    total_size: totalSize,
    errors: session.errors,
    duration_ms: durationMs,
    fs_total: fsStats ? fsStats[0] : null,
    fs_free: fsStats ? fsStats[1] : null,
  };
}

async function scanDirectoryParallel(pathValue, threads) {
  const start = Date.now();
  const fsAdapter = process.platform === 'win32' ? new WindowsFsAdapter() : new UnixFsAdapter();
  const session = new Session(pathValue, 10);
  const fsStats = getFilesystemStats(pathValue);

  await session.runParallel(fsAdapter, threads || 16);

  const durationMs = Date.now() - start;
  const rootId = session.tree.rootId();
  const totalSize = session.tree.node(rootId).totalBytes();

  return {
    tree: serializeTree(session.tree, rootId),
    root_id: rootId,
    total_size: totalSize,
    errors: session.errors,
    duration_ms: durationMs,
    fs_total: fsStats ? fsStats[0] : null,
    fs_free: fsStats ? fsStats[1] : null,
  };
}

async function scanDirectoryParallelStream(pathValue, threads, sender) {
  const start = Date.now();
  const fsAdapter = process.platform === 'win32' ? new WindowsFsAdapter() : new UnixFsAdapter();
  const session = new Session(pathValue, 10);
  const fsStats = getFilesystemStats(pathValue);
  const fsTotal = fsStats ? fsStats[0] : null;
  const fsFree = fsStats ? fsStats[1] : null;

  await session.runParallelWithCallback(fsAdapter, threads || 16, (sess) => {
    const rootId = sess.tree.rootId();
    const currentSize = sess.tree.node(rootId).totalBytes();
    const payload = {
      jobs_started: sess.jobsStarted,
      jobs_done: sess.jobsDone,
      errors: sess.errors,
      current_size: currentSize,
      duration_ms: Date.now() - start,
      tree: serializeTreeLimited(sess.tree, rootId, 2),
      root_id: rootId,
      fs_total: fsTotal,
      fs_free: fsFree,
    };

    sender.send('event:scan_progress', payload);
  });

  const durationMs = Date.now() - start;
  const rootId = session.tree.rootId();
  const totalSize = session.tree.node(rootId).totalBytes();

  const result = {
    tree: serializeTree(session.tree, rootId),
    root_id: rootId,
    total_size: totalSize,
    errors: session.errors,
    duration_ms: durationMs,
    fs_total: fsTotal,
    fs_free: fsFree,
  };

  sender.send('event:scan_done', result);

  return result;
}

function createWindow() {
  const window = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  const uiPath = path.join(__dirname, '..', 'ui', 'index.html');
  window.loadFile(uiPath);
}

ipcMain.handle('dialog:open', async (_event, options) => {
  const result = await dialog.showOpenDialog({
    properties: ['openDirectory'],
    title: options && options.title ? options.title : 'Select Directory',
  });

  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }

  return result.filePaths[0];
});

ipcMain.handle('tauri:invoke', async (event, payload) => {
  const command = payload && payload.cmd ? payload.cmd : '';
  const args = payload && payload.args ? payload.args : {};

  if (command === 'scan_directory') {
    return scanDirectory(args.path);
  }

  if (command === 'scan_directory_parallel') {
    return scanDirectoryParallel(args.path, args.threads);
  }

  if (command === 'scan_directory_parallel_stream') {
    return scanDirectoryParallelStream(args.path, args.threads, event.sender);
  }

  throw new Error(`Unknown command: ${command}`);
});

app.whenReady().then(() => {
  createWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
