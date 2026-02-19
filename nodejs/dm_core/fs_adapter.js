const fs = require('fs/promises');

class UnixFsAdapter {
  async readDir(path) {
    return fs.readdir(path, { withFileTypes: true });
  }

  async isTraversableDir(dirent) {
    return dirent.isDirectory() && !dirent.isSymbolicLink();
  }
}

class WindowsFsAdapter {
  async readDir(path) {
    return fs.readdir(path, { withFileTypes: true });
  }

  async isTraversableDir(dirent, fullPath) {
    if (!dirent.isDirectory()) {
      return false;
    }
    if (dirent.isSymbolicLink()) {
      return false;
    }

    try {
      const stats = await fs.lstat(fullPath);
      if (stats.isSymbolicLink()) {
        return false;
      }
    } catch (error) {
      throw error;
    }

    return true;
  }
}

module.exports = {
  UnixFsAdapter,
  WindowsFsAdapter,
};
