// Import Tauri API
const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

console.log('Tauri API loaded:', { invoke, open });

let currentData = null;
let currentRoot = 0;
let nodeMap = new Map();

// DOM elements
const pathInput = document.getElementById('pathInput');
const browseBtn = document.getElementById('browseBtn');
const scanBtn = document.getElementById('scanBtn');
const parallelScan = document.getElementById('parallelScan');
const threadCount = document.getElementById('threadCount');
const showFreeSpace = document.getElementById('showFreeSpace');
const statusMessage = document.getElementById('statusMessage');
const stats = document.getElementById('stats');
const totalSize = document.getElementById('totalSize');
const freeSpace = document.getElementById('freeSpace');
const fsTotal = document.getElementById('fsTotal');
const duration = document.getElementById('duration');
const errors = document.getElementById('errors');
const treemap = document.getElementById('treemap');
const breadcrumb = document.getElementById('breadcrumb');
const tooltip = document.getElementById('tooltip');

// Format bytes to human readable
function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return (bytes / Math.pow(k, i)).toFixed(2) + ' ' + sizes[i];
}

// Color scale for treemap
function getColor(depth) {
    const colors = [
        '#2196F3', // Blue
        '#4CAF50', // Green
        '#FFEB3B', // Yellow
        '#F44336', // Red
        '#9C27B0', // Magenta
        '#00BCD4', // Cyan
        '#FF9800', // Orange
        '#673AB7', // Purple
    ];
    return colors[depth % colors.length];
}

// Build node map
function buildNodeMap(tree) {
    nodeMap.clear();
    console.log('Building node map from tree with', tree.length, 'nodes');
    tree.forEach(node => {
        // Ensure size and id are present
        if (node.id !== undefined) {
            nodeMap.set(node.id, node);
        }
    });
    console.log('Node map built, size:', nodeMap.size);
}

// Get path from root to node
function getPath(nodeId) {
    const path = [];
    let currentId = nodeId;

    while (currentId !== undefined && currentId !== null) {
        const current = nodeMap.get(currentId);
        if (!current) break;
        path.unshift(current);
        currentId = current.parent;
    }

    return path;
}

// Render breadcrumb
function renderBreadcrumb(nodeId) {
    const path = getPath(nodeId);

    if (path.length <= 1) {
        breadcrumb.classList.add('hidden');
        return;
    }

    breadcrumb.classList.remove('hidden');
    breadcrumb.innerHTML = path.map((node, idx) =>
        `<span data-id="${node.id}">${node.name || 'root'}</span>`
    ).join('');

    // Add click handlers
    breadcrumb.querySelectorAll('span').forEach(span => {
        span.addEventListener('click', () => {
            const id = parseInt(span.dataset.id);
            currentRoot = id;
            renderTreemap();
        });
    });
}

// Calculate treemap layout using a simple biased algorithm
function layoutChildren(children, x, y, width, height) {
    if (children.length === 0 || width <= 0 || height <= 0) return { layout: [], skipped: [] };

    const totalSize = children.reduce((sum, child) => sum + child.size, 0);
    if (totalSize === 0) return { layout: [], skipped: [] };

    let currentX = x;
    let currentY = y;
    let remainingWidth = width;
    let remainingHeight = height;
    let remainingTotalSize = totalSize;
    const layout = [];
    const skipped = [];

    for (let i = 0; i < children.length; i++) {
        const child = children[i];
        if (remainingWidth <= 0 || remainingHeight <= 0 || remainingTotalSize <= 0) {
            skipped.push(...children.slice(i));
            break;
        }

        const ratio = child.size / remainingTotalSize;
        let itemWidth, itemHeight;

        // Bias towards wide short containers: horizontal split if width >= height * 1.5
        if (remainingWidth >= remainingHeight * 1.5) {
            itemWidth = remainingWidth * ratio;
            itemHeight = remainingHeight;
        } else {
            itemWidth = remainingWidth;
            itemHeight = remainingHeight * ratio;
        }

        // Clamp to remaining space
        itemWidth = Math.min(itemWidth, remainingWidth);
        itemHeight = Math.min(itemHeight, remainingHeight);

        if (itemWidth < 1 || itemHeight < 1) {
            skipped.push(child);
        } else {
            layout.push({
                ...child,
                x: currentX,
                y: currentY,
                width: itemWidth,
                height: itemHeight
            });
        }

        // Update remaining space
        if (remainingWidth >= remainingHeight * 1.5) {
            currentX += itemWidth;
            remainingWidth -= itemWidth;
        } else {
            currentY += itemHeight;
            remainingHeight -= itemHeight;
        }
        remainingTotalSize -= child.size;
    }

    return { layout, skipped };
}

// Render treemap
function renderTreemap() {
    if (!currentData) return;

    const rootNode = nodeMap.get(currentRoot);
    if (!rootNode) {
        console.error('Root node not found:', currentRoot);
        return;
    }

    // Clear SVG
    treemap.innerHTML = '';

    const rect = treemap.getBoundingClientRect();
    const width = Math.max(rect.width, 100);
    const height = Math.max(rect.height, 100);

    treemap.setAttribute('viewBox', `0 0 ${width} ${height}`);
    treemap.setAttribute('preserveAspectRatio', 'none');

    // Check if we're at the actual root and have filesystem stats
    const isAtActualRoot = currentRoot === currentData.root_id;
    const hasFsStats = currentData.fs_total && currentData.fs_free;

    if (isAtActualRoot && hasFsStats) {
        // Render with free space visualization
        const usedBytes = rootNode.size;
        const freeBytes = currentData.fs_free;
        const totalBytes = currentData.fs_total;

        // Calculate proportional widths
        const usedRatio = usedBytes / totalBytes;
        const usedWidth = Math.max(width * usedRatio, width * 0.2); // At least 20% for used
        const freeWidth = width - usedWidth;

        // Render used space (the tree structure)
        if (usedWidth >= 40) {
            renderNode(currentRoot, 0, 0, usedWidth, height, 0);
        }

        // Render free space box
        if (freeWidth >= 40) {
            renderFreeSpaceBox(usedWidth, 0, freeWidth, height, freeBytes);
        }
    } else {
        // Normal render without free space
        renderNode(currentRoot, 0, 0, width, height, 0);
    }

    renderBreadcrumb(currentRoot);
}

// Render free space as a box
function renderFreeSpaceBox(x, y, width, height, freeBytes) {
    const g = document.createElementNS('http://www.w3.org/2000/svg', 'g');

    // Main rectangle
    const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
    rect.setAttribute('x', x);
    rect.setAttribute('y', y);
    rect.setAttribute('width', width);
    rect.setAttribute('height', height);
    rect.setAttribute('fill', '#555555');
    rect.setAttribute('fill-opacity', '0.3');
    rect.setAttribute('stroke', '#888888');
    rect.setAttribute('stroke-width', '3');
    rect.setAttribute('class', 'treemap-rect');

    // Add hover handler
    rect.addEventListener('mouseenter', (e) => {
        tooltip.classList.remove('hidden');
        tooltip.innerHTML = `<div class="name">Free Space</div>
            <div class="size">Available: ${formatBytes(freeBytes)}</div>`;
    });

    rect.addEventListener('mousemove', (e) => {
        tooltip.style.left = (e.clientX + 10) + 'px';
        tooltip.style.top = (e.clientY + 10) + 'px';
    });

    rect.addEventListener('mouseleave', () => {
        tooltip.classList.add('hidden');
    });

    g.appendChild(rect);

    // Add label if space permits
    if (width > 80 && height > 40) {
        const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
        text.setAttribute('x', x + width / 2);
        text.setAttribute('y', y + height / 2 - 10);
        text.setAttribute('text-anchor', 'middle');
        text.setAttribute('class', 'treemap-text');
        text.style.fontSize = '16px';
        text.style.fill = '#cccccc';
        text.textContent = 'Free Space';
        g.appendChild(text);

        const sizeText = document.createElementNS('http://www.w3.org/2000/svg', 'text');
        sizeText.setAttribute('x', x + width / 2);
        sizeText.setAttribute('y', y + height / 2 + 10);
        sizeText.setAttribute('text-anchor', 'middle');
        sizeText.setAttribute('class', 'treemap-text');
        sizeText.style.fontSize = '14px';
        sizeText.style.fill = '#aaaaaa';
        sizeText.textContent = formatBytes(freeBytes);
        g.appendChild(sizeText);
    }

    treemap.appendChild(g);
}

function renderNode(nodeId, x, y, width, height, depth) {
    const node = nodeMap.get(nodeId);
    if (!node || width < 1 || height < 1) return;

    // Get children and files
    const items = [
        ...node.children
            .map(id => {
                const childNode = nodeMap.get(id);
                return childNode ? { ...childNode, isFolder: true } : null;
            })
            .filter(n => n && n.size > 0),
        ...node.top_files.map(f => ({ ...f, isFile: true, depth: node.depth + 1 }))
    ].sort((a, b) => b.size - a.size);

    const { layout, skipped } = layoutChildren(items, x, y, width, height);

    layout.forEach(item => {
        const g = document.createElementNS('http://www.w3.org/2000/svg', 'g');

        const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
        rect.setAttribute('x', item.x);
        rect.setAttribute('y', item.y);
        rect.setAttribute('width', item.width);
        rect.setAttribute('height', item.height);

        // Color by type and depth
        if (item.isFile) {
            rect.setAttribute('fill', getColor(item.depth));
            rect.setAttribute('fill-opacity', '0.4');
            rect.setAttribute('stroke', getColor(item.depth));
        } else {
            rect.setAttribute('fill', getColor(item.depth));
            rect.setAttribute('stroke', 'rgba(0,0,0,0.2)');
        }
        rect.setAttribute('class', 'treemap-rect');

        // Add click handler for folders
        if (item.isFolder) {
            rect.addEventListener('click', (e) => {
                e.stopPropagation();
                if (item.children && item.children.length > 0) {
                    currentRoot = item.id;
                    renderTreemap();
                }
            });
        }

        // Add hover handlers
        rect.addEventListener('mouseenter', (e) => {
            tooltip.classList.remove('hidden');
            let content = `<div class="name">${item.name}</div>
                <div class="size">Size: ${formatBytes(item.size)}</div>`;
            if (item.isFolder) {
                content += `<div class="size">Own: ${formatBytes(item.own_size)}</div>`;
            }
            tooltip.innerHTML = content;
        });

        rect.addEventListener('mousemove', (e) => {
            tooltip.style.left = (e.clientX + 10) + 'px';
            tooltip.style.top = (e.clientY + 10) + 'px';
        });

        rect.addEventListener('mouseleave', () => {
            tooltip.classList.add('hidden');
        });

        g.appendChild(rect);

        // Add text label if space permits
        if (item.width > 30 && item.height > 15) {
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('x', item.x + 3);
            text.setAttribute('y', item.y + 12);
            text.setAttribute('class', 'treemap-text');
            if (item.isFile) text.style.fontSize = '10px';

            const label = item.name.length > 20
                ? item.name.substring(0, 17) + '..'
                : item.name;
            const sizeStr = formatBytes(item.size);

            // Show size next to name on same line
            if (item.width > 100) {
                text.textContent = `${label} (${sizeStr})`;
            } else {
                text.textContent = label;
            }
            g.appendChild(text);
        }

        treemap.appendChild(g);

        // Recursively render folder children if it's a folder and has space
        if (item.isFolder && item.width > 40 && item.height > 40 && depth < 32) {
            // Add a small margin for nested content
            const margin = 2;
            const labelHeight = 15;
            renderNode(
                item.id,
                item.x + margin,
                item.y + labelHeight,
                item.width - margin * 2,
                item.height - labelHeight - margin,
                depth + 1
            );
        }
    });

    // Add summary if items were skipped (only at the level we're currently rendering)
    if (skipped.length > 0 && depth === 0) {
        const fileCount = skipped.filter(i => i.isFile).length;
        const folders = skipped.filter(i => i.isFolder).length;

        if (folders > 0 || fileCount > 0) {
            const summaryText = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            summaryText.setAttribute('x', 5);
            summaryText.setAttribute('y', height - 5);
            summaryText.setAttribute('class', 'treemap-text');
            summaryText.style.fill = '#aaa';
            summaryText.style.fontSize = '11px';
            let msg = `... and ${folders} more folders, ${fileCount} more files`;
            summaryText.textContent = msg;
            treemap.appendChild(summaryText);
        }
    }
}

// Browse directory
browseBtn.addEventListener('click', async () => {
    console.log('Browse button clicked');
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            title: 'Select Directory to Analyze'
        });

        if (selected) {
            pathInput.value = selected;
        }
    } catch (error) {
        console.error('Error opening directory dialog:', error);
        statusMessage.textContent = 'Error opening directory dialog: ' + error;
        statusMessage.className = 'error';
    }
});

// Scan directory
scanBtn.addEventListener('click', async () => {
    console.log('Scan button clicked');
    const path = pathInput.value.trim();
    if (!path) {
        statusMessage.textContent = 'Please enter a directory path';
        statusMessage.className = 'error';
        return;
    }

    scanBtn.disabled = true;
    statusMessage.textContent = 'Scanning directory...';
    statusMessage.className = 'scanning';
    stats.classList.add('hidden');

    try {
        const isParallel = parallelScan.checked;
        const threads = parseInt(threadCount.value) || 4;

        console.log('Invoking scan:', { path, isParallel, threads });

        const result = isParallel
            ? await invoke('scan_directory_parallel', { path, threads })
            : await invoke('scan_directory', { path });

        console.log('Scan result:', result);

        currentData = result;
        currentRoot = result.root_id;
        buildNodeMap(result.tree);

        // Update stats
        totalSize.textContent = formatBytes(result.total_size);
        freeSpace.textContent = result.fs_free ? formatBytes(result.fs_free) : 'N/A';
        fsTotal.textContent = result.fs_total ? formatBytes(result.fs_total) : 'N/A';
        duration.textContent = result.duration_ms + ' ms';
        errors.textContent = result.errors;
        stats.classList.remove('hidden');

        statusMessage.textContent = 'Scan completed successfully';
        statusMessage.className = 'success';

        // Render treemap
        renderTreemap();

    } catch (error) {
        statusMessage.textContent = 'Error: ' + error;
        statusMessage.className = 'error';
        console.error('Scan error:', error);
    } finally {
        scanBtn.disabled = false;
    }
});

// Handle window resize
window.addEventListener('resize', () => {
    if (currentData) {
        renderTreemap();
    }
});

// Enable parallel scan by default
parallelScan.checked = true;

// Set default path
async function setDefaultPath() {
    if (!pathInput.value) {
        try {
            // We can check the platform via Tauri
            const platform = window.__TAURI__.core.platform; // Note: This might need adjustment depending on Tauri version
            // For now, let's use a simple approach: if it's Windows-like, use C:\, else /
            // A better way is to ask the backend or use a known constant.
            // Since we don't have a direct platform API here without more plugins,
            // we'll assume / and let the backend/browse handle it, or just use a common default.
            
            // Actually, let's just use the root / as a safe cross-platform starting point that
            // most OSs understand or can be easily edited.
            // Wait, the requirement said "root of the system drive".
            
            // Let's use a small hack to detect Windows
            const isWindows = navigator.userAgent.includes('Windows');
            pathInput.value = isWindows ? 'C:\\' : '/';
        } catch (e) {
            pathInput.value = '/';
        }
    }
}

setDefaultPath();
console.log('Event listeners set up');

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
    // Enter or Space to start scanning
    if (e.key === 'Enter' || e.key === ' ') {
        // Don't trigger if user is typing in the input field
        if (document.activeElement === pathInput) {
            return;
        }
        e.preventDefault();
        scanBtn.click();
    }

    // Plus or Equals to zoom in (drill down into selected folder)
    if (e.key === '+' || e.key === '=') {
        e.preventDefault();
        // Find the largest child folder and zoom into it
        if (currentData && currentRoot !== null) {
            const rootNode = nodeMap.get(currentRoot);
            if (rootNode && rootNode.children && rootNode.children.length > 0) {
                // Find largest child by size
                let largestChild = null;
                let largestSize = 0;
                for (const childId of rootNode.children) {
                    const child = nodeMap.get(childId);
                    if (child && child.size > largestSize) {
                        largestSize = child.size;
                        largestChild = childId;
                    }
                }
                if (largestChild !== null) {
                    currentRoot = largestChild;
                    renderTreemap();
                }
            }
        }
    }

    // Minus or Dash to zoom out (go to parent)
    if (e.key === '-' || e.key === '_') {
        e.preventDefault();
        zoomOut();
    }

    // Backspace to zoom out one level
    if (e.key === 'Backspace') {
        // Don't trigger if user is typing in the input field
        if (document.activeElement === pathInput) {
            return;
        }
        e.preventDefault();
        zoomOut();
    }
});

// Helper function to zoom out
function zoomOut() {
    if (currentData && currentRoot !== null) {
        const currentNode = nodeMap.get(currentRoot);
        if (currentNode && currentNode.parent !== null && currentNode.parent !== undefined) {
            currentRoot = currentNode.parent;
            renderTreemap();
        }
    }
}
