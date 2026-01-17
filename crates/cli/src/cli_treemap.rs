use dm_core::tree::Tree;
use dm_core::model::NodeId;
use dm_core::top_files::FileMetaData;

#[derive(Clone)]
enum LayoutItem {
    Folder(NodeId),
    File(FileMetaData),
}

impl LayoutItem {
    fn size(&self, tree: &Tree) -> u64 {
        match self {
            LayoutItem::Folder(id) => tree.node(*id).total_bytes(),
            LayoutItem::File(f) => f.size,
        }
    }

    fn clone_item(&self) -> Self {
        match self {
            LayoutItem::Folder(id) => LayoutItem::Folder(*id),
            LayoutItem::File(f) => LayoutItem::File(f.clone()),
        }
    }
}

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
        _total_bytes: u64,
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

            let mut used_width = (self.width as f64 * used_ratio).round() as usize;
            let mut free_width = self.width.saturating_sub(used_width);

            // Ensure both sections get minimum visibility if they have data
            const MIN_WIDTH: usize = 15;
            if used_bytes > 0 && used_width < MIN_WIDTH {
                used_width = MIN_WIDTH.min(self.width - MIN_WIDTH);
                free_width = self.width.saturating_sub(used_width);
            }
            if available_bytes > 0 && free_width < MIN_WIDTH {
                free_width = MIN_WIDTH.min(self.width - MIN_WIDTH);
                used_width = self.width.saturating_sub(free_width);
            }

            (used_width, free_width)
        } else {
            // Non-proportional: split width evenly
            let half_width = self.width / 2;
            (half_width, self.width.saturating_sub(half_width))
        };

        // Render used space (the tree structure)
        if used_width >= 3 {
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
        if free_width >= 3 {
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
        let mut next_content_y = y + 1;
        if width > 4 && height > 2 {
            let name = node.name();
            let size = node.total_bytes();
            let label_width = width.saturating_sub(4);
            let size_label = Self::format_size(size);

            // Calculate space needed for both name and size on same line
            let size_with_parens = format!(" ({})", size_label);
            let available_for_name = label_width.saturating_sub(size_with_parens.len());

            let name_label = Self::truncate_name(name, available_for_name);
            let label = format!("{}{}", name_label, size_with_parens);

            self.draw_text(canvas, x + 2, next_content_y, &label, label_width, depth);
            next_content_y += 1;
        }

        // Prepare layout items (folders and files)
        let mut items = Vec::new();
        for &child_id in children {
            items.push(LayoutItem::Folder(child_id));
        }
        let top_files = node.top_files().to_sorted_vec_desc();
        for file in top_files {
            items.push(LayoutItem::File(file));
        }

        // Calculate layout for children and files
        if !items.is_empty() && height > (next_content_y - y + 2) && width > 4 {
            let inner_x = x.saturating_add(2);
            let inner_y = next_content_y;
            let inner_width = width.saturating_sub(4);
            let inner_height = height.saturating_sub(next_content_y - y + 2); // Reserve 1 line for summary if needed

            if inner_width > 0 && inner_height > 0 {
                let skipped = self.layout_children(
                    tree,
                    &items,
                    canvas,
                    inner_x,
                    inner_y,
                    inner_width,
                    inner_height,
                    depth + 1,
                );

                // Removed file summary at bottom - just skip items that don't fit
                let _ = skipped;
            }
        }
    }

    fn render_file(
        &self,
        canvas: &mut Vec<Vec<Cell>>,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        depth: usize,
        name: &str,
        size: u64,
    ) {
        if width < 1 || height < 1 {
            return;
        }

        if width >= 5 && height >= 3 {
            self.draw_box(canvas, x, y, width, height, depth);
            let label_width = width.saturating_sub(4);
            let size_label = Self::format_size(size);

            // Calculate space needed for both name and size on same line
            let size_with_parens = format!(" ({})", size_label);
            let available_for_name = label_width.saturating_sub(size_with_parens.len());

            let name_label = Self::truncate_name(name, available_for_name);
            let label = format!("{}{}", name_label, size_with_parens);

            self.draw_text(canvas, x + 2, y + 1, &label, label_width, depth);
        } else {
            // Fill with shaded characters for small files if they can't fit text
            let fill_ch = match depth % 4 {
                0 => '░',
                1 => '▒',
                2 => '▓',
                _ => '█',
            };
            for row in y..(y + height) {
                for col in x..(x + width) {
                    if row < canvas.len() && col < canvas[0].len() {
                        // For small blocks, only fill if there's no text already there
                        if canvas[row][col].ch == ' ' {
                            canvas[row][col] = if self.use_colors {
                                Cell::with_color(fill_ch, depth)
                            } else {
                                Cell::new(fill_ch)
                            };
                        }
                    }
                }
            }
            // Try to overlay name if possible
            if width >= 5 && height >= 1 {
                let label = Self::truncate_name(name, width.saturating_sub(2));
                let label = format!("- {}", label);
                self.draw_text(canvas, x, y, &label, width, depth);
            } else if width >= 2 && height >= 1 {
                // Just try to show a few chars
                let label = Self::truncate_name(name, width);
                self.draw_text(canvas, x, y, &label, width, depth);
            }
        }
    }

    fn layout_children(
        &self,
        tree: &Tree,
        items: &[LayoutItem],
        canvas: &mut Vec<Vec<Cell>>,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        depth: usize,
    ) -> Vec<LayoutItem> {
        if items.is_empty() || width == 0 || height == 0 {
            return Vec::new();
        }

        // Calculate total size of items we are actually going to display
        let mut sorted_items: Vec<(&LayoutItem, u64)> = items
            .iter()
            .map(|item| (item, item.size(tree)))
            .filter(|(_, size)| *size > 0)
            .collect();

        sorted_items.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by size descending

        let total_display_size: u64 = sorted_items.iter().map(|(_, size)| *size).sum();

        if total_display_size == 0 {
            return Vec::new();
        }

        let mut current_x = x;
        let mut current_y = y;
        let mut remaining_width = width;
        let mut remaining_height = height;
        let mut remaining_total_size = total_display_size;
        let mut skipped_items = Vec::new();

        for (item_idx, (item, item_size)) in sorted_items.iter().enumerate() {
            if remaining_width == 0 || remaining_height == 0 || remaining_total_size == 0 {
                skipped_items.extend(sorted_items.iter().skip(item_idx).map(|(it, _)| (*it).clone_item()));
                break;
            }

            let ratio = *item_size as f64 / remaining_total_size as f64;

            // Decide direction based on aspect ratio of remaining space
            // Terminal characters are ~2:1 (height:width) visually.
            // Biasing heavily towards wide containers (horizontal split) when width is larger than height.
            let (item_width, item_height) = if (remaining_width as f64) > (remaining_height as f64 * 1.5) {
                // Split horizontally (side-by-side blocks)
                let w = (remaining_width as f64 * ratio).round() as usize;
                let w = w.clamp(1, remaining_width);
                (w, remaining_height)
            } else {
                // Split vertically (stacked blocks)
                let h = (remaining_height as f64 * ratio).round() as usize;
                let h = h.clamp(1, remaining_height);
                (remaining_width, h)
            };

            // Check if it's too small to render anything meaningful
            if item_width < 1 || item_height < 1 {
                skipped_items.push((*item).clone_item());
            } else {
                match item {
                    LayoutItem::Folder(child_id) => {
                        self.render_node(
                            tree,
                            *child_id,
                            canvas,
                            current_x,
                            current_y,
                            item_width,
                            item_height,
                            depth,
                        );
                    }
                    LayoutItem::File(file) => {
                        self.render_file(
                            canvas,
                            current_x,
                            current_y,
                            item_width,
                            item_height,
                            depth,
                            &file.name,
                            file.size,
                        );
                    }
                }
            }

            // Update position for next child based on the SAME split decision
            if (remaining_width as f64) > (remaining_height as f64 * 1.5) {
                current_x += item_width;
                remaining_width = remaining_width.saturating_sub(item_width);
            } else {
                current_y += item_height;
                remaining_height = remaining_height.saturating_sub(item_height);
            }
            remaining_total_size = remaining_total_size.saturating_sub(*item_size);
        }

        skipped_items
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
        } else if max_len < 1 {
            "".to_string()
        } else if max_len < 3 {
            name.chars().take(max_len).collect()
        } else {
            // Prefer showing the beginning of the name
            let take = max_len.saturating_sub(2);
            format!("{}..", name.chars().take(take).collect::<String>())
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
