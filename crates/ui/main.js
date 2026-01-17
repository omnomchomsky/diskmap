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
const status = document.getElementById('status');
const stats = document.getElementById('stats');
const totalSize = document.getElementById('totalSize');
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
function getColor(depth, index, total) {
    const hue = (index / total) * 360;
    const lightness = 50 - (depth * 5);
    return `hsl(${hue}, 70%, ${lightness}%)`;
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

// Calculate treemap layout using squarified algorithm
function squarify(children, x, y, width, height) {
    if (children.length === 0) return [];

    const totalSize = children.reduce((sum, child) => sum + child.size, 0);
    if (totalSize === 0) return [];

    // Normalize sizes
    const area = width * height;
    children = children.map(child => ({
        ...child,
        normalizedSize: (child.size / totalSize) * area
    }));

    const rows = [];
    let remaining = [...children];

    while (remaining.length > 0) {
        const isWide = width >= height;
        const length = isWide ? width : height;

        // Find best row
        let row = [remaining[0]];
        let rowSum = row[0].normalizedSize;
        let bestRatio = getWorstRatio(row, rowSum, length);

        for (let i = 1; i < remaining.length; i++) {
            const newRow = [...row, remaining[i]];
            const newSum = rowSum + remaining[i].normalizedSize;
            const newRatio = getWorstRatio(newRow, newSum, length);

            if (newRatio <= bestRatio) {
                row = newRow;
                rowSum = newSum;
                bestRatio = newRatio;
            } else {
                break;
            }
        }

        // Layout this row
        const breadth = rowSum / length;
        const rowCoords = layoutRow(row, x, y, length, breadth, isWide);
        rows.push(...rowCoords);

        // Update position and remaining
        if (isWide) {
            x += breadth;
            width -= breadth;
        } else {
            y += breadth;
            height -= breadth;
        }

        remaining = remaining.slice(row.length);
    }

    return rows;
}

function getWorstRatio(row, rowSum, length) {
    const breadth = rowSum / length;
    let maxRatio = 0;

    for (const item of row) {
        const rectLength = item.normalizedSize / breadth;
        const ratio = Math.max(rectLength / breadth, breadth / rectLength);
        maxRatio = Math.max(maxRatio, ratio);
    }

    return maxRatio;
}

function layoutRow(row, x, y, length, breadth, isWide) {
    const coords = [];
    let offset = 0;

    for (const item of row) {
        const size = item.normalizedSize / breadth;

        if (isWide) {
            coords.push({
                ...item,
                x: x,
                y: y + offset,
                width: breadth,
                height: size
            });
        } else {
            coords.push({
                ...item,
                x: x + offset,
                y: y,
                width: size,
                height: breadth
            });
        }

        offset += size;
    }

    return coords;
}

// Render treemap
function renderTreemap() {
    if (!currentData) return;

    const rootNode = nodeMap.get(currentRoot);
    if (!rootNode) {
        console.error('Root node not found:', currentRoot);
        return;
    }

    console.log('Root node:', rootNode);
    console.log('Root node children IDs:', rootNode.children);

    // Get children with sizes
    const children = rootNode.children
        .map(id => {
            const node = nodeMap.get(id);
            if (!node) console.warn('Child node not found:', id);
            return node;
        })
        .filter(node => node && node.size > 0)
        .sort((a, b) => b.size - a.size);

    console.log('Children to render:', children.length, children);

    // Calculate layout
    const rect = treemap.getBoundingClientRect();
    const width = Math.max(rect.width, 100);
    const height = Math.max(rect.height, 100);

    treemap.setAttribute('viewBox', `0 0 ${width} ${height}`);
    treemap.setAttribute('preserveAspectRatio', 'none');

    const layout = squarify(children, 0, 0, width, height);

    console.log('Layout computed, items:', layout.length);

    // Clear SVG
    treemap.innerHTML = '';

    // Render rectangles
    let renderedCount = 0;
    layout.forEach((item, index) => {
        if (item.width < 1 || item.height < 1) {
            console.log('Skipping tiny item:', item.name, 'size:', formatBytes(item.size), 'dimensions:', item.width, 'x', item.height);
            return;
        }
        renderedCount++;

        const g = document.createElementNS('http://www.w3.org/2000/svg', 'g');

        const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
        rect.setAttribute('x', item.x);
        rect.setAttribute('y', item.y);
        rect.setAttribute('width', item.width);
        rect.setAttribute('height', item.height);
        rect.setAttribute('fill', getColor(1, index, layout.length));
        rect.setAttribute('class', 'treemap-rect');

        // Add click handler
        rect.addEventListener('click', () => {
            if (item.children && item.children.length > 0) {
                currentRoot = item.id;
                renderTreemap();
            }
        });

        // Add hover handlers
        rect.addEventListener('mouseenter', (e) => {
            tooltip.classList.remove('hidden');
            tooltip.innerHTML = `
                <div class="name">${item.name}</div>
                <div class="size">Size: ${formatBytes(item.size)}</div>
                <div class="size">Own: ${formatBytes(item.own_size)}</div>
            `;
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
        if (item.width > 40 && item.height > 20) {
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('x', item.x + 5);
            text.setAttribute('y', item.y + 15);
            text.setAttribute('class', 'treemap-text');
            text.textContent = item.name.length > 20
                ? item.name.substring(0, 17) + '...'
                : item.name;
            g.appendChild(text);

            if (item.height > 35) {
                const sizeText = document.createElementNS('http://www.w3.org/2000/svg', 'text');
                sizeText.setAttribute('x', item.x + 5);
                sizeText.setAttribute('y', item.y + 30);
                sizeText.setAttribute('class', 'treemap-text');
                sizeText.setAttribute('opacity', '0.7');
                sizeText.textContent = formatBytes(item.size);
                g.appendChild(sizeText);
            }
        }

        treemap.appendChild(g);
    });

    console.log('Rendered', renderedCount, 'of', layout.length, 'items');

    renderBreadcrumb(currentRoot);
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
        status.textContent = 'Error opening directory dialog: ' + error;
        status.className = 'status error';
    }
});

// Scan directory
scanBtn.addEventListener('click', async () => {
    console.log('Scan button clicked');
    const path = pathInput.value.trim();
    if (!path) {
        status.textContent = 'Please enter a directory path';
        status.className = 'status error';
        return;
    }

    scanBtn.disabled = true;
    status.textContent = 'Scanning directory...';
    status.className = 'status scanning';
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
        duration.textContent = result.duration_ms + ' ms';
        errors.textContent = result.errors;
        stats.classList.remove('hidden');

        status.textContent = 'Scan completed successfully';
        status.className = 'status success';

        // Render treemap
        renderTreemap();

    } catch (error) {
        status.textContent = 'Error: ' + error;
        status.className = 'status error';
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
if (pathInput.value === '/') {
    pathInput.value = '';
}

console.log('Event listeners set up');
