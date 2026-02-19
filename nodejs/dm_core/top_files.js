function clampToSafeInteger(value) {
  if (value >= Number.MAX_SAFE_INTEGER) {
    return Number.MAX_SAFE_INTEGER;
  }
  return value;
}

class FileMetaData {
  constructor(name, size, mtime = 0) {
    this.name = name;
    this.size = clampToSafeInteger(size);
    this.mtime = mtime;
  }
}

class TopKFiles {
  constructor(k) {
    this.k = k;
    this.items = [];
  }

  offer(name, size) {
    if (this.k === 0) {
      return;
    }

    const entry = new FileMetaData(name, size, 0);

    if (this.items.length < this.k) {
      this.items.push(entry);
      return;
    }

    let minIndex = 0;
    let minSize = this.items[0].size;
    for (let i = 1; i < this.items.length; i += 1) {
      if (this.items[i].size < minSize) {
        minSize = this.items[i].size;
        minIndex = i;
      }
    }

    if (entry.size < minSize) {
      return;
    }

    this.items[minIndex] = entry;
  }

  toSortedVecDesc() {
    const sorted = this.items.map((item) => new FileMetaData(item.name, item.size, item.mtime));
    sorted.sort((a, b) => b.size - a.size);
    return sorted;
  }
}

module.exports = {
  FileMetaData,
  TopKFiles,
  clampToSafeInteger,
};
