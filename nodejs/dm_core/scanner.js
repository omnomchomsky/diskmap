const fs = require('fs/promises');
const path = require('path');

async function fileSizeForEntry(fullPath) {
  const stats = await fs.stat(fullPath);

  if (process.platform === 'win32') {
    if (typeof stats.isSymbolicLink === 'function' && stats.isSymbolicLink()) {
      return null;
    }
    const len = stats.size;
    if (len === 0) {
      return 0;
    }
    return Math.ceil(len / 4096) * 4096;
  }

  if (stats.nlink && stats.nlink > 1) {
    return null;
  }

  if (typeof stats.blocks === 'number') {
    return stats.blocks * 512;
  }

  const len = stats.size;
  if (len === 0) {
    return 0;
  }
  return Math.ceil(len / 4096) * 4096;
}

async function scanOneDir(fsAdapter, job, emit) {
  const parentId = job.nodeId;
  let entries;

  try {
    entries = await fsAdapter.readDir(job.path);
  } catch (error) {
    emit({ type: 'Error', node: parentId, path: job.path, kind: error.code || error.name });
    emit({ type: 'Done', node: parentId });
    return;
  }

  for (const entry of entries) {
    const name = entry.name;
    const entryPath = path.join(job.path, name);

    if (entry.isSymbolicLink()) {
      continue;
    }

    if (entry.isFile()) {
      let size;
      try {
        size = await fileSizeForEntry(entryPath);
      } catch (error) {
        emit({ type: 'Error', node: parentId, path: entryPath, kind: error.code || error.name });
        continue;
      }
      if (size === null) {
        continue;
      }
      emit({ type: 'File', parentId, name, size });
      continue;
    }

    if (entry.isDirectory()) {
      let traversable;
      try {
        traversable = await fsAdapter.isTraversableDir(entry, entryPath);
      } catch (error) {
        emit({ type: 'Error', node: parentId, path: entryPath, kind: error.code || error.name });
        continue;
      }

      if (traversable) {
        emit({ type: 'Dir', parentId, name, path: entryPath });
      }
    }
  }

  emit({ type: 'Done', node: parentId });
}

module.exports = {
  scanOneDir,
};
