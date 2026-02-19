const { FileMetaData } = require('../dm_core/top_files');

const COLORS = [
  '\u001b[38;5;33m',
  '\u001b[38;5;46m',
  '\u001b[38;5;226m',
  '\u001b[38;5;196m',
  '\u001b[38;5;165m',
  '\u001b[38;5;51m',
  '\u001b[38;5;208m',
  '\u001b[38;5;99m',
];
const RESET = '\u001b[0m';

class Cell {
  constructor(ch, colorIndex = null) {
    this.ch = ch;
    this.colorIndex = colorIndex;
  }

  static new(ch) {
    return new Cell(ch, null);
  }

  static withColor(ch, colorIndex) {
    return new Cell(ch, colorIndex);
  }
}

class TreeMapView {
  constructor(width, height) {
    this.width = width;
    this.height = height;
    this.useColors = true;
  }

  withColors(useColors) {
    this.useColors = useColors;
    return this;
  }

  getColor(depth) {
    if (!this.useColors) {
      return '';
    }
    return COLORS[depth % COLORS.length];
  }

  renderTree(tree, fsStats, proportional) {
    const rootId = tree.rootId();
    const canvas = Array.from({ length: this.height }, () =>
      Array.from({ length: this.width }, () => Cell.new(' '))
    );

    if (fsStats) {
      const [totalBytes, availableBytes] = fsStats;
      const usedBytes = tree.node(rootId).totalBytes();
      this.renderWithFreeSpace(
        tree,
        rootId,
        canvas,
        usedBytes,
        availableBytes,
        totalBytes,
        proportional
      );
    } else {
      this.renderNode(tree, rootId, canvas, 0, 0, this.width, this.height, 0);
    }

    return this.canvasToString(canvas);
  }

  canvasToString(canvas) {
    let result = '';
    let currentColor = null;

    for (const row of canvas) {
      for (const cell of row) {
        if (this.useColors) {
          if (cell.colorIndex !== currentColor) {
            if (cell.colorIndex !== null && cell.colorIndex !== undefined) {
              result += COLORS[cell.colorIndex % COLORS.length];
            } else {
              result += RESET;
            }
            currentColor = cell.colorIndex;
          }
        }
        result += cell.ch;
      }
      if (this.useColors && currentColor !== null) {
        result += RESET;
        currentColor = null;
      }
      result += '\n';
    }

    return result;
  }

  renderWithFreeSpace(
    tree,
    rootId,
    canvas,
    usedBytes,
    availableBytes,
    totalBytes,
    proportional
  ) {
    let usedWidth;
    let freeWidth;

    if (proportional) {
      const comparisonTotal = usedBytes + availableBytes;
      const usedRatio = comparisonTotal > 0 ? usedBytes / comparisonTotal : 0.5;
      usedWidth = Math.round(this.width * usedRatio);
      freeWidth = Math.max(0, this.width - usedWidth);

      const MIN_WIDTH = 15;
      if (usedBytes > 0 && usedWidth < MIN_WIDTH) {
        usedWidth = Math.min(MIN_WIDTH, this.width - MIN_WIDTH);
        freeWidth = Math.max(0, this.width - usedWidth);
      }
      if (availableBytes > 0 && freeWidth < MIN_WIDTH) {
        freeWidth = Math.min(MIN_WIDTH, this.width - MIN_WIDTH);
        usedWidth = Math.max(0, this.width - freeWidth);
      }
    } else {
      const halfWidth = Math.floor(this.width / 2);
      usedWidth = halfWidth;
      freeWidth = Math.max(0, this.width - halfWidth);
    }

    if (usedWidth >= 3) {
      this.renderNode(tree, rootId, canvas, 0, 0, usedWidth, this.height, 0);
    }

    if (freeWidth >= 3) {
      const freeX = usedWidth;
      this.drawFreeSpaceBox(canvas, freeX, 0, freeWidth, this.height, availableBytes);
    }
  }

  drawFreeSpaceBox(canvas, x, y, width, height, freeBytes) {
    if (width < 3 || height < 3) {
      return;
    }

    const colorIdx = 7;
    this.drawBox(canvas, x, y, width, height, colorIdx);

    if (width > 10 && height > 2) {
      const label = `Free (${TreeMapView.formatSize(freeBytes)})`;
      const labelWidth = Math.max(0, width - 4);
      this.drawText(canvas, x + 2, y + 1, label, labelWidth, colorIdx);
    }
  }

  renderNode(tree, nodeId, canvas, x, y, width, height, depth) {
    if (width < 3 || height < 3) {
      return;
    }

    const node = tree.node(nodeId);
    const children = node.childrenValue();

    this.drawBox(canvas, x, y, width, height, depth);

    let nextContentY = y + 1;
    if (width > 4 && height > 2) {
      const name = node.nameValue();
      const size = node.totalBytes();
      const labelWidth = Math.max(0, width - 4);
      const sizeLabel = TreeMapView.formatSize(size);
      const sizeWithParens = ` (${sizeLabel})`;
      const availableForName = Math.max(0, labelWidth - sizeWithParens.length);
      const nameLabel = TreeMapView.truncateName(name, availableForName);
      const label = `${nameLabel}${sizeWithParens}`;
      this.drawText(canvas, x + 2, nextContentY, label, labelWidth, depth);
      nextContentY += 1;
    }

    const items = [];
    for (const childId of children) {
      items.push({ kind: 'folder', id: childId });
    }
    const topFiles = node.topFiles().toSortedVecDesc();
    for (const file of topFiles) {
      items.push({ kind: 'file', file });
    }

    const innerHeight = height - (nextContentY - y + 2);
    if (items.length > 0 && height > nextContentY - y + 2 && width > 4) {
      const innerX = x + 2;
      const innerY = nextContentY;
      const innerWidth = Math.max(0, width - 4);

      if (innerWidth > 0 && innerHeight > 0) {
        this.layoutChildren(
          tree,
          items,
          canvas,
          innerX,
          innerY,
          innerWidth,
          innerHeight,
          depth + 1
        );
      }
    }
  }

  renderFile(canvas, x, y, width, height, depth, name, size) {
    if (width < 1 || height < 1) {
      return;
    }

    if (width >= 5 && height >= 3) {
      this.drawBox(canvas, x, y, width, height, depth);
      const labelWidth = Math.max(0, width - 4);
      const sizeLabel = TreeMapView.formatSize(size);
      const sizeWithParens = ` (${sizeLabel})`;
      const availableForName = Math.max(0, labelWidth - sizeWithParens.length);
      const nameLabel = TreeMapView.truncateName(name, availableForName);
      const label = `${nameLabel}${sizeWithParens}`;
      this.drawText(canvas, x + 2, y + 1, label, labelWidth, depth);
    } else {
      const fillCh = (() => {
        switch (depth % 4) {
          case 0:
            return '\u2591';
          case 1:
            return '\u2592';
          case 2:
            return '\u2593';
          default:
            return '\u2588';
        }
      })();

      for (let row = y; row < y + height; row += 1) {
        for (let col = x; col < x + width; col += 1) {
          if (row < canvas.length && col < canvas[0].length) {
            if (canvas[row][col].ch === ' ') {
              canvas[row][col] = this.useColors
                ? Cell.withColor(fillCh, depth)
                : Cell.new(fillCh);
            }
          }
        }
      }

      if (width >= 5 && height >= 1) {
        const label = TreeMapView.truncateName(name, Math.max(0, width - 2));
        const labelWithDash = `- ${label}`;
        this.drawText(canvas, x, y, labelWithDash, width, depth);
      } else if (width >= 2 && height >= 1) {
        const label = TreeMapView.truncateName(name, width);
        this.drawText(canvas, x, y, label, width, depth);
      }
    }
  }

  layoutChildren(tree, items, canvas, x, y, width, height, depth) {
    if (items.length === 0 || width === 0 || height === 0) {
      return [];
    }

    const sizedItems = items
      .map((item) => ({
        item,
        size: item.kind === 'folder' ? tree.node(item.id).totalBytes() : item.file.size,
      }))
      .filter((entry) => entry.size > 0)
      .sort((a, b) => b.size - a.size);

    const totalDisplaySize = sizedItems.reduce((sum, entry) => sum + entry.size, 0);
    if (totalDisplaySize === 0) {
      return [];
    }

    let currentX = x;
    let currentY = y;
    let remainingWidth = width;
    let remainingHeight = height;
    let remainingTotalSize = totalDisplaySize;
    const skipped = [];

    for (let i = 0; i < sizedItems.length; i += 1) {
      const { item, size } = sizedItems[i];

      if (remainingWidth === 0 || remainingHeight === 0 || remainingTotalSize === 0) {
        skipped.push(...sizedItems.slice(i).map((entry) => entry.item));
        break;
      }

      const ratio = size / remainingTotalSize;
      let itemWidth;
      let itemHeight;

      if (remainingWidth > remainingHeight * 1.5) {
        itemWidth = Math.round(remainingWidth * ratio);
        itemWidth = Math.max(1, Math.min(itemWidth, remainingWidth));
        itemHeight = remainingHeight;
      } else {
        itemHeight = Math.round(remainingHeight * ratio);
        itemHeight = Math.max(1, Math.min(itemHeight, remainingHeight));
        itemWidth = remainingWidth;
      }

      if (itemWidth < 1 || itemHeight < 1) {
        skipped.push(item);
      } else if (item.kind === 'folder') {
        this.renderNode(tree, item.id, canvas, currentX, currentY, itemWidth, itemHeight, depth);
      } else if (item.kind === 'file') {
        this.renderFile(
          canvas,
          currentX,
          currentY,
          itemWidth,
          itemHeight,
          depth,
          item.file.name,
          item.file.size
        );
      }

      if (remainingWidth > remainingHeight * 1.5) {
        currentX += itemWidth;
        remainingWidth = Math.max(0, remainingWidth - itemWidth);
      } else {
        currentY += itemHeight;
        remainingHeight = Math.max(0, remainingHeight - itemHeight);
      }
      remainingTotalSize = Math.max(0, remainingTotalSize - size);
    }

    return skipped;
  }

  drawBox(canvas, x, y, width, height, depth) {
    if (width < 2 || height < 2) {
      return;
    }

    const styles = [
      ['\u250c', '\u2510', '\u2514', '\u2518', '\u2500', '\u2502'],
      ['\u2554', '\u2557', '\u255a', '\u255d', '\u2550', '\u2551'],
      ['\u256d', '\u256e', '\u2570', '\u256f', '\u2500', '\u2502'],
      ['\u250f', '\u2513', '\u2517', '\u251b', '\u2501', '\u2503'],
    ];

    const [tl, tr, bl, br, h, v] = styles[depth % 4];

    const maxX = Math.min(x + width - 1, canvas[0].length - 1);
    const maxY = Math.min(y + height - 1, canvas.length - 1);

    const colorIndex = this.useColors ? depth : null;

    for (let i = x + 1; i < maxX; i += 1) {
      if (y < canvas.length && i < canvas[0].length) {
        canvas[y][i] = colorIndex !== null ? Cell.withColor(h, colorIndex) : Cell.new(h);
      }
      if (maxY < canvas.length && i < canvas[0].length) {
        canvas[maxY][i] = colorIndex !== null ? Cell.withColor(h, colorIndex) : Cell.new(h);
      }
    }

    for (let i = y + 1; i < maxY; i += 1) {
      if (i < canvas.length && x < canvas[0].length) {
        canvas[i][x] = colorIndex !== null ? Cell.withColor(v, colorIndex) : Cell.new(v);
      }
      if (i < canvas.length && maxX < canvas[0].length) {
        canvas[i][maxX] = colorIndex !== null ? Cell.withColor(v, colorIndex) : Cell.new(v);
      }
    }

    if (y < canvas.length && x < canvas[0].length) {
      canvas[y][x] = colorIndex !== null ? Cell.withColor(tl, colorIndex) : Cell.new(tl);
    }
    if (y < canvas.length && maxX < canvas[0].length) {
      canvas[y][maxX] = colorIndex !== null ? Cell.withColor(tr, colorIndex) : Cell.new(tr);
    }
    if (maxY < canvas.length && x < canvas[0].length) {
      canvas[maxY][x] = colorIndex !== null ? Cell.withColor(bl, colorIndex) : Cell.new(bl);
    }
    if (maxY < canvas.length && maxX < canvas[0].length) {
      canvas[maxY][maxX] = colorIndex !== null ? Cell.withColor(br, colorIndex) : Cell.new(br);
    }
  }

  drawText(canvas, x, y, text, maxWidth, depth) {
    if (y >= canvas.length || x >= canvas[0].length || maxWidth === 0) {
      return;
    }

    const availableWidth = Math.max(0, canvas[0].length - x);
    const writeWidth = Math.min(maxWidth, availableWidth);

    let idx = 0;
    for (const ch of text) {
      if (idx >= writeWidth) {
        break;
      }
      const pos = x + idx;
      if (pos < canvas[0].length) {
        canvas[y][pos] = this.useColors ? Cell.withColor(ch, depth) : Cell.new(ch);
      }
      idx += 1;
    }
  }

  static truncateName(name, maxLen) {
    if (name.length <= maxLen) {
      return name;
    }
    if (maxLen < 1) {
      return '';
    }
    if (maxLen < 3) {
      return name.slice(0, maxLen);
    }
    const take = Math.max(0, maxLen - 2);
    return `${name.slice(0, take)}..`;
  }

  static formatSize(bytes) {
    const KB = 1024;
    const MB = KB * 1024;
    const GB = MB * 1024;
    const TB = GB * 1024;

    if (bytes >= TB) {
      return `${(bytes / TB).toFixed(1)}T`;
    }
    if (bytes >= GB) {
      return `${(bytes / GB).toFixed(1)}G`;
    }
    if (bytes >= MB) {
      return `${(bytes / MB).toFixed(1)}M`;
    }
    if (bytes >= KB) {
      return `${(bytes / KB).toFixed(1)}K`;
    }
    return `${bytes}B`;
  }
}

module.exports = {
  TreeMapView,
};
