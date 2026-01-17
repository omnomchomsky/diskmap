use dm_core::tree::Tree;
use dm_core::model::NodeId;

/// ANSI color codes for different depths
const COLORS: &[&str] = &[
    "\x1b[38;5;33m",  // Blue
    "\x1b[38;5;46m",  // Green
    "\x1b[38;5;226m", // Yellow
    "\x1b[38;5;196m", // Red
    "\x1b[38;5;165m", // Magenta
    "\x1b[38;5;51m",  // Cyan
    "\x1b[38;5;208m", // Orange
    "\x1b[38;5;99m",  // Purple
];
const RESET: &str = "\x1b[0m";

/// Cell in the canvas that can hold color info
#[derive(Clone)]
struct Cell {
    ch: char,
    color_index: Option<usize>,
}

impl Cell {
    fn new(ch: char) -> Self {
        Self { ch, color_index: None }
    }

    fn with_color(ch: char, color_index: usize) -> Self {
        Self { ch, color_index: Some(color_index) }
    }
}

/// ASCII treemap visualizer that renders nested rectangles
pub struct TreeMapView {
    width: usize,
    height: usize,
    use_colors: bool,
}

impl TreeMapView {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            use_colors: true,
        }
    }

    pub fn with_colors(mut self, use_colors: bool) -> Self {
        self.use_colors = use_colors;
        self
    }

    fn get_color(&self, depth: usize) -> &'static str {
        if !self.use_colors {
            return "";
        }
        COLORS[depth % COLORS.len()]
    }

    /// Renders a treemap visualization
    /// fs_stats is optional (total_bytes, available_bytes) for showing free space
    /// proportional determines if blocks are sized according to actual disk usage
    pub fn render_tree(&self, tree: &Tree, fs_stats: Option<(u64, u64)>, proportional: bool) -> String {
        let root_id = tree.root();

        // Create a canvas with color info
        let mut canvas = vec![vec![Cell::new(' '); self.width]; self.height];

        // If we have fs_stats, we need to show both used and free space
        if let Some((total_bytes, available_bytes)) = fs_stats {
            let used_bytes = tree.node(root_id).total_bytes();

            // Render as two side-by-side sections
            self.render_with_free_space(
                tree,
                root_id,
                &mut canvas,
                used_bytes,
                available_bytes,
                total_bytes,
                proportional,
            );
        } else {
            // Render the tree recursively (normal mode)
            self.render_node(
                tree,
                root_id,
                &mut canvas,
                0,
                0,
                self.width,
                self.height,
                0,
            );
        }

        // Convert canvas to colored string
        self.canvas_to_string(&canvas)
    }

    fn canvas_to_string(&self, canvas: &[Vec<Cell>]) -> String {
        let mut result = String::new();
        let mut current_color: Option<usize> = None;

        for row in canvas {
            for cell in row {
                if self.use_colors {
                    if cell.color_index != current_color {
                        // Color changed, emit new color code
                        if let Some(color_idx) = cell.color_index {
                            result.push_str(COLORS[color_idx % COLORS.len()]);
                        } else {
                            result.push_str(RESET);
                        }
                        current_color = cell.color_index;
                    }
                }
                result.push(cell.ch);
            }
            if self.use_colors && current_color.is_some() {
                result.push_str(RESET);
                current_color = None;
            }
            result.push('\n');
        }

        result
    }

    fn render_with_free_space(
        &self,
        tree: &Tree,
        root_id: NodeId,
        canvas: &mut Vec<Vec<Cell>>,
        used_bytes: u64,
        available_bytes: u64,
        total_bytes: u64,
        proportional: bool,
    ) {
        let (used_width, free_width) = if proportional {
            // Calculate proportional widths based on used vs available
            let comparison_total = used_bytes + available_bytes;
            let used_ratio = if comparison_total > 0 {
                used_bytes as f64 / comparison_total as f64
            } else {
                0.5
            };

            let used_width = (self.width as f64 * used_ratio).ceil().max(1.0) as usize;
            let free_width = self.width.saturating_sub(used_width);

            (used_width, free_width)
        } else {
            // Non-proportional: split width evenly
            let half_width = self.width / 2;
            (half_width, self.width.saturating_sub(half_width))
        };

        // Render used space (the tree structure)
        if used_width > 3 {
            self.render_node(
                tree,
                root_id,
                canvas,
                0,
                0,
                used_width,
                self.height,
                0,
            );
        }

        // Render free space as a simple box
        if free_width > 3 {
            let free_x = used_width;
            self.draw_free_space_box(canvas, free_x, 0, free_width, self.height, available_bytes);
        }
    }

    fn draw_free_space_box(
        &self,
        canvas: &mut Vec<Vec<Cell>>,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        free_bytes: u64,
    ) {
        if width < 3 || height < 3 {
            return;
        }

        // Use a gray color for free space (color index 7 = purple, let's use that)
        let color_idx = 7; // Or we could add a new gray color

        // Draw border
        self.draw_box(canvas, x, y, width, height, color_idx);

        // Add label
        if width > 10 && height > 2 {
            let label = format!("Free ({})", Self::format_size(free_bytes));
            let label_width = width.saturating_sub(4);
            self.draw_text(canvas, x + 2, y + 1, &label, label_width, color_idx);
        }
    }

    fn render_node(
        &self,
        tree: &Tree,
        node_id: NodeId,
        canvas: &mut Vec<Vec<Cell>>,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        depth: usize,
    ) {
        if width < 3 || height < 3 {
            return;
        }

        let node = tree.node(node_id);
        let children = node.children();

        // Draw border with color
        self.draw_box(canvas, x, y, width, height, depth);

        // Add label (only if there's enough space)
        if width > 4 && height > 2 {
            let name = node.name();
            let size = node.total_bytes();
            let label_width = width.saturating_sub(4);
            let label = format!("{} ({})", Self::truncate_name(name, label_width), Self::format_size(size));
            self.draw_text(canvas, x + 2, y + 1, &label, label_width, depth);
        }

        // Calculate layout for children
        if !children.is_empty() && height > 5 && width > 6 {
            let inner_x = x.saturating_add(2).min(canvas[0].len().saturating_sub(1));
            let inner_y = y.saturating_add(3).min(canvas.len().saturating_sub(1));
            let inner_width = width.saturating_sub(4);
            let inner_height = height.saturating_sub(5);

            if inner_width > 0 && inner_height > 0 {
                self.layout_children(
                    tree,
                    children,
                    canvas,
                    inner_x,
                    inner_y,
                    inner_width,
                    inner_height,
                    depth + 1,
                );
            }
        }
    }

    fn layout_children(
        &self,
        tree: &Tree,
        children: &[NodeId],
        canvas: &mut Vec<Vec<Cell>>,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        depth: usize,
    ) {
        if children.is_empty() || width == 0 || height == 0 {
            return;
        }

        // Calculate total size
        let total_size: u64 = children
            .iter()
            .map(|&id| tree.node(id).total_bytes())
            .sum();

        if total_size == 0 {
            return;
        }

        // Use squarified treemap algorithm (simplified)
        let mut remaining_children: Vec<(NodeId, u64)> = children
            .iter()
            .map(|&id| (id, tree.node(id).total_bytes()))
            .filter(|(_, size)| *size > 0)
            .collect();

        remaining_children.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by size descending

        let mut current_x = x;
        let mut current_y = y;
        let mut remaining_width = width;
        let mut remaining_height = height;

        for (child_id, child_size) in remaining_children {
            if remaining_width == 0 || remaining_height == 0 {
                break;
            }

            let ratio = child_size as f64 / total_size as f64;

            // Decide direction based on aspect ratio
            let (child_width, child_height) = if remaining_width > remaining_height {
                // Split horizontally
                let w = (remaining_width as f64 * ratio).max(1.0).min(remaining_width as f64) as usize;
                let w = w.clamp(1, remaining_width);
                (w, remaining_height)
            } else {
                // Split vertically
                let h = (remaining_height as f64 * ratio).max(1.0).min(remaining_height as f64) as usize;
                let h = h.clamp(1, remaining_height);
                (remaining_width, h)
            };

            self.render_node(
                tree,
                child_id,
                canvas,
                current_x,
                current_y,
                child_width,
                child_height,
                depth,
            );

            // Update position for next child
            if remaining_width > remaining_height {
                current_x += child_width;
                remaining_width = remaining_width.saturating_sub(child_width);
            } else {
                current_y += child_height;
                remaining_height = remaining_height.saturating_sub(child_height);
            }
        }
    }

    fn draw_box(
        &self,
        canvas: &mut Vec<Vec<Cell>>,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        depth: usize,
    ) {
        if width < 2 || height < 2 {
            return;
        }

        // Use different border styles for different depths
        let (tl, tr, bl, br, h, v) = match depth % 4 {
            0 => ('┌', '┐', '└', '┘', '─', '│'),
            1 => ('╔', '╗', '╚', '╝', '═', '║'),
            2 => ('╭', '╮', '╰', '╯', '─', '│'),
            _ => ('┏', '┓', '┗', '┛', '━', '┃'),
        };

        let max_x = (x + width - 1).min(canvas[0].len() - 1);
        let max_y = (y + height - 1).min(canvas.len() - 1);

        let color_index = if self.use_colors { Some(depth) } else { None };

        // Top and bottom
        for i in (x + 1)..max_x {
            if y < canvas.len() && i < canvas[0].len() {
                canvas[y][i] = if let Some(ci) = color_index {
                    Cell::with_color(h, ci)
                } else {
                    Cell::new(h)
                };
            }
            if max_y < canvas.len() && i < canvas[0].len() {
                canvas[max_y][i] = if let Some(ci) = color_index {
                    Cell::with_color(h, ci)
                } else {
                    Cell::new(h)
                };
            }
        }

        // Left and right
        for i in (y + 1)..max_y {
            if i < canvas.len() && x < canvas[0].len() {
                canvas[i][x] = if let Some(ci) = color_index {
                    Cell::with_color(v, ci)
                } else {
                    Cell::new(v)
                };
            }
            if i < canvas.len() && max_x < canvas[0].len() {
                canvas[i][max_x] = if let Some(ci) = color_index {
                    Cell::with_color(v, ci)
                } else {
                    Cell::new(v)
                };
            }
        }

        // Corners
        if y < canvas.len() && x < canvas[0].len() {
            canvas[y][x] = if let Some(ci) = color_index {
                Cell::with_color(tl, ci)
            } else {
                Cell::new(tl)
            };
        }
        if y < canvas.len() && max_x < canvas[0].len() {
            canvas[y][max_x] = if let Some(ci) = color_index {
                Cell::with_color(tr, ci)
            } else {
                Cell::new(tr)
            };
        }
        if max_y < canvas.len() && x < canvas[0].len() {
            canvas[max_y][x] = if let Some(ci) = color_index {
                Cell::with_color(bl, ci)
            } else {
                Cell::new(bl)
            };
        }
        if max_y < canvas.len() && max_x < canvas[0].len() {
            canvas[max_y][max_x] = if let Some(ci) = color_index {
                Cell::with_color(br, ci)
            } else {
                Cell::new(br)
            };
        }
    }

    fn draw_text(
        &self,
        canvas: &mut Vec<Vec<Cell>>,
        x: usize,
        y: usize,
        text: &str,
        max_width: usize,
        depth: usize,
    ) {
        if y >= canvas.len() || x >= canvas[0].len() || max_width == 0 {
            return;
        }

        let available_width = canvas[0].len().saturating_sub(x);
        let write_width = max_width.min(available_width);

        for (i, ch) in text.chars().take(write_width).enumerate() {
            let pos = x + i;
            if pos < canvas[0].len() {
                canvas[y][pos] = if self.use_colors {
                    Cell::with_color(ch, depth)
                } else {
                    Cell::new(ch)
                };
            }
        }
    }

    fn truncate_name(name: &str, max_len: usize) -> String {
        if name.len() <= max_len {
            name.to_string()
        } else if max_len < 3 {
            name.chars().take(max_len).collect()
        } else {
            format!("{}...", name.chars().take(max_len - 3).collect::<String>())
        }
    }

    fn format_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        if bytes >= TB {
            format!("{:.1}T", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.1}G", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1}M", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1}K", bytes as f64 / KB as f64)
        } else {
            format!("{}B", bytes)
        }
    }
}

impl Default for TreeMapView {
    fn default() -> Self {
        Self::new(120, 40)
    }
}
