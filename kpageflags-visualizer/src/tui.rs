use crate::{get_category_symbol_and_color, FlagCategory, KPageFlagsReader, PageInfo, PAGE_FLAGS};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct AppState {
    pub pages: Vec<PageInfo>,
    pub zoom_level: f64,
    pub offset_x: i64,
    pub offset_y: i64,
    pub grid_width: usize,
    pub grid_height: usize,
    pub selected_page: Option<usize>,
    pub show_help: bool,
    pub show_stats: bool,
    pub filter_category: Option<FlagCategory>,
    pub last_update: Instant,
    pub total_pages_scanned: usize,
    pub scanning: bool,
    pub scan_progress: f64,
    // Mouse selection state
    pub mouse_selecting: bool,
    pub selection_start: Option<(u16, u16)>,
    pub selection_end: Option<(u16, u16)>,
    pub grid_area: Option<Rect>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            zoom_level: 1.0,
            offset_x: 0,
            offset_y: 0,
            grid_width: 80,
            grid_height: 24,
            selected_page: None,
            show_help: false,
            show_stats: true,
            filter_category: None,
            last_update: Instant::now(),
            total_pages_scanned: 0,
            scanning: false,
            scan_progress: 0.0,
            mouse_selecting: false,
            selection_start: None,
            selection_end: None,
            grid_area: None,
        }
    }
}

pub struct TuiApp {
    state: AppState,
    reader: KPageFlagsReader,
    interrupt_flag: Arc<AtomicBool>,
}

impl TuiApp {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let reader = KPageFlagsReader::new()?;
        let interrupt_flag = Arc::new(AtomicBool::new(false));

        Ok(Self {
            state: AppState::default(),
            reader,
            interrupt_flag,
        })
    }

    pub async fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Start background scanning
        self.start_background_scan().await?;

        loop {
            terminal.draw(|f| self.ui(f))?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('q') => break,
                                KeyCode::Char('h') => self.state.show_help = !self.state.show_help,
                                KeyCode::Char('s') => {
                                    self.state.show_stats = !self.state.show_stats
                                }
                                KeyCode::Char('r') => self.refresh_data().await?,
                                KeyCode::Char('+') | KeyCode::Char('=') => self.zoom_in(),
                                KeyCode::Char('-') => self.zoom_out(),
                                KeyCode::Up => self.move_up(),
                                KeyCode::Down => self.move_down(),
                                KeyCode::Left => self.move_left(),
                                KeyCode::Right => self.move_right(),
                                KeyCode::Char('1') => self.set_filter(Some(FlagCategory::State)),
                                KeyCode::Char('2') => self.set_filter(Some(FlagCategory::Memory)),
                                KeyCode::Char('3') => self.set_filter(Some(FlagCategory::Usage)),
                                KeyCode::Char('4') => {
                                    self.set_filter(Some(FlagCategory::Allocation))
                                }
                                KeyCode::Char('5') => self.set_filter(Some(FlagCategory::IO)),
                                KeyCode::Char('6') => {
                                    self.set_filter(Some(FlagCategory::Structure))
                                }
                                KeyCode::Char('7') => self.set_filter(Some(FlagCategory::Special)),
                                KeyCode::Char('8') => self.set_filter(Some(FlagCategory::Error)),
                                KeyCode::Char('0') => self.set_filter(None),
                                KeyCode::Home => self.reset_view(),
                                KeyCode::Esc => self.cancel_selection(),
                                _ => {}
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse_event(mouse);
                    }
                    _ => {}
                }
            }

            // Update scan progress if scanning
            if self.state.scanning {
                self.update_scan_progress().await?;
            }

            sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }

    async fn start_background_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.state.scanning = true;
        self.state.scan_progress = 0.0;

        // Start with a small sample for immediate feedback
        let initial_pages = self
            .reader
            .read_range(0, 10000, self.interrupt_flag.clone())?;
        self.state.pages = initial_pages;
        self.state.total_pages_scanned = self.state.pages.len();
        self.state.scan_progress = 0.01; // Start at 1%

        Ok(())
    }

    async fn update_scan_progress(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Simulate progressive scanning
        if self.state.scan_progress < 1.0 {
            self.state.scan_progress += 0.01;

            // Load more pages as we progress
            if self.state.scan_progress > 0.5 && self.state.pages.len() < 50000 {
                let more_pages = self.reader.read_range(
                    self.state.pages.len() as u64,
                    10000,
                    self.interrupt_flag.clone(),
                )?;
                self.state.pages.extend(more_pages);
                self.state.total_pages_scanned = self.state.pages.len();
            }
        } else {
            self.state.scanning = false;
        }

        Ok(())
    }

    async fn refresh_data(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.state.scanning = true;
        self.state.scan_progress = 0.0;

        // Reload data
        let pages = self
            .reader
            .read_range(0, 100000, self.interrupt_flag.clone())?;
        self.state.pages = pages;
        self.state.total_pages_scanned = self.state.pages.len();
        self.state.last_update = Instant::now();
        self.state.scanning = false;
        self.state.scan_progress = 1.0;

        Ok(())
    }

    fn zoom_in(&mut self) {
        self.state.zoom_level = (self.state.zoom_level * 1.2).min(10.0);
    }

    fn zoom_out(&mut self) {
        self.state.zoom_level = (self.state.zoom_level / 1.2).max(0.1);
    }

    fn move_up(&mut self) {
        self.state.offset_y -= (10.0 / self.state.zoom_level) as i64;
    }

    fn move_down(&mut self) {
        self.state.offset_y += (10.0 / self.state.zoom_level) as i64;
    }

    fn move_left(&mut self) {
        self.state.offset_x -= (10.0 / self.state.zoom_level) as i64;
    }

    fn move_right(&mut self) {
        self.state.offset_x += (10.0 / self.state.zoom_level) as i64;
    }

    fn set_filter(&mut self, category: Option<FlagCategory>) {
        self.state.filter_category = category;
    }

    fn reset_view(&mut self) {
        self.state.zoom_level = 1.0;
        self.state.offset_x = 0;
        self.state.offset_y = 0;
    }

    fn cancel_selection(&mut self) {
        self.state.mouse_selecting = false;
        self.state.selection_start = None;
        self.state.selection_end = None;
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if let Some(grid_area) = self.state.grid_area {
            // Check if mouse is within grid area
            if mouse.column >= grid_area.x
                && mouse.column < grid_area.x + grid_area.width
                && mouse.row >= grid_area.y
                && mouse.row < grid_area.y + grid_area.height
            {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        self.state.mouse_selecting = true;
                        self.state.selection_start = Some((mouse.column, mouse.row));
                        self.state.selection_end = Some((mouse.column, mouse.row));
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if self.state.mouse_selecting {
                            self.state.selection_end = Some((mouse.column, mouse.row));
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        if self.state.mouse_selecting {
                            self.state.selection_end = Some((mouse.column, mouse.row));
                            self.zoom_to_selection();
                            self.cancel_selection();
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        self.zoom_in();
                    }
                    MouseEventKind::ScrollDown => {
                        self.zoom_out();
                    }
                    _ => {}
                }
            }
        }
    }

    fn zoom_to_selection(&mut self) {
        if let (Some(start), Some(end), Some(grid_area)) = (
            self.state.selection_start,
            self.state.selection_end,
            self.state.grid_area,
        ) {
            // Calculate selection bounds relative to grid
            let grid_start_x = start.0.saturating_sub(grid_area.x);
            let grid_start_y = start.1.saturating_sub(grid_area.y);
            let grid_end_x = end.0.saturating_sub(grid_area.x);
            let grid_end_y = end.1.saturating_sub(grid_area.y);

            let min_x = grid_start_x.min(grid_end_x) as i64;
            let max_x = grid_start_x.max(grid_end_x) as i64;
            let min_y = grid_start_y.min(grid_end_y) as i64;
            let max_y = grid_start_y.max(grid_end_y) as i64;

            // Calculate selection dimensions
            let selection_width = (max_x - min_x + 1) as f64;
            let selection_height = (max_y - min_y + 1) as f64;

            if selection_width > 1.0 && selection_height > 1.0 {
                // Calculate zoom factor to fit selection to screen
                let zoom_x = grid_area.width as f64 / selection_width;
                let zoom_y = grid_area.height as f64 / selection_height;
                let new_zoom = zoom_x.min(zoom_y).min(10.0).max(0.1);

                // Update zoom and center on selection
                self.state.zoom_level = new_zoom;

                // Convert grid coordinates to page coordinates
                let pages_per_row =
                    ((grid_area.width as f64 * self.state.zoom_level) as usize).max(1);
                let center_x = (min_x + max_x) / 2;
                let center_y = (min_y + max_y) / 2;

                // Adjust offset to center the selection
                self.state.offset_x =
                    (center_x as f64 / self.state.zoom_level) as i64 - (grid_area.width as i64 / 2);
                self.state.offset_y = (center_y as f64 / self.state.zoom_level) as i64
                    - (grid_area.height as i64 / 2);

                // Ensure offsets don't go negative
                self.state.offset_x = self.state.offset_x.max(0);
                self.state.offset_y = self.state.offset_y.max(0);
            }
        }
    }

    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(3), // Footer
            ])
            .split(f.size());

        // Header
        self.render_header(f, chunks[0]);

        // Main content
        if self.state.show_help {
            self.render_help(f, chunks[1]);
        } else {
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(70), // Grid
                    Constraint::Percentage(30), // Stats
                ])
                .split(chunks[1]);

            self.render_grid(f, main_chunks[0]);

            if self.state.show_stats {
                self.render_stats(f, main_chunks[1]);
            }
        }

        // Footer
        self.render_footer(f, chunks[2]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let title = if self.state.scanning {
            format!(
                "KPageFlags TUI - Scanning... ({:.1}%) - {} pages loaded",
                self.state.scan_progress * 100.0,
                self.state.total_pages_scanned
            )
        } else {
            format!(
                "KPageFlags TUI - {} pages loaded - Zoom: {:.1}x",
                self.state.total_pages_scanned, self.state.zoom_level
            )
        };

        let header = Paragraph::new(title)
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(header, area);

        // Progress bar if scanning
        if self.state.scanning {
            let progress_area = Rect {
                x: area.x + 2,
                y: area.y + 2,
                width: area.width - 4,
                height: 1,
            };

            let progress = Gauge::default()
                .block(Block::default())
                .gauge_style(Style::default().fg(Color::Green))
                .ratio(self.state.scan_progress.min(1.0).max(0.0));

            f.render_widget(progress, progress_area);
        }
    }

    fn render_grid(&mut self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Memory Page Grid (Click and drag to zoom)")
            .borders(Borders::ALL);

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Store grid area for mouse handling
        self.state.grid_area = Some(inner);

        // Calculate grid dimensions based on zoom and area
        let grid_width = ((inner.width as f64 * self.state.zoom_level) as usize).max(1);
        let grid_height = ((inner.height as f64 * self.state.zoom_level) as usize).max(1);

        // Create grid content
        let mut lines = Vec::new();
        let pages_per_row = grid_width;

        let filtered_pages: Vec<&PageInfo> = if let Some(filter_cat) = self.state.filter_category {
            self.state
                .pages
                .iter()
                .filter(|page| page.get_flag_categories().contains(&filter_cat))
                .collect()
        } else {
            self.state.pages.iter().collect()
        };

        let start_idx = (self.state.offset_y * pages_per_row as i64 + self.state.offset_x) as usize;

        for row in 0..grid_height.min(inner.height as usize) {
            let mut spans = Vec::new();

            for col in 0..pages_per_row.min(inner.width as usize) {
                let page_idx = start_idx + row * pages_per_row + col;

                let (symbol, mut color) = if page_idx < filtered_pages.len() {
                    let page = filtered_pages[page_idx];
                    self.get_page_symbol_and_color(page)
                } else {
                    ('.', Color::DarkGray)
                };

                // Check if this cell is in the selection
                if self.is_cell_in_selection(inner, col as u16, row as u16) {
                    // Highlight selected cells with inverted colors
                    color = Color::Black;
                    spans.push(Span::styled(
                        symbol.to_string(),
                        Style::default().fg(color).bg(Color::White),
                    ));
                } else {
                    spans.push(Span::styled(symbol.to_string(), Style::default().fg(color)));
                }
            }

            lines.push(Line::from(spans));
        }

        let grid_text = Text::from(lines);
        let grid_paragraph = Paragraph::new(grid_text).wrap(Wrap { trim: false });

        f.render_widget(grid_paragraph, inner);

        // Render selection overlay if selecting
        if self.state.mouse_selecting {
            self.render_selection_overlay(f, inner);
        }
    }

    fn is_cell_in_selection(&self, grid_area: Rect, col: u16, row: u16) -> bool {
        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end) {
            let grid_start_x = start.0.saturating_sub(grid_area.x);
            let grid_start_y = start.1.saturating_sub(grid_area.y);
            let grid_end_x = end.0.saturating_sub(grid_area.x);
            let grid_end_y = end.1.saturating_sub(grid_area.y);

            let min_x = grid_start_x.min(grid_end_x);
            let max_x = grid_start_x.max(grid_end_x);
            let min_y = grid_start_y.min(grid_end_y);
            let max_y = grid_start_y.max(grid_end_y);

            col >= min_x && col <= max_x && row >= min_y && row <= max_y
        } else {
            false
        }
    }

    fn render_selection_overlay(&self, f: &mut Frame, grid_area: Rect) {
        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end) {
            let grid_start_x = start.0.saturating_sub(grid_area.x);
            let grid_start_y = start.1.saturating_sub(grid_area.y);
            let grid_end_x = end.0.saturating_sub(grid_area.x);
            let grid_end_y = end.1.saturating_sub(grid_area.y);

            let min_x = grid_start_x.min(grid_end_x);
            let max_x = grid_start_x.max(grid_end_x);
            let min_y = grid_start_y.min(grid_end_y);
            let max_y = grid_start_y.max(grid_end_y);

            // Create selection info text
            let selection_info = format!(
                "Selection: {}x{} ({}x{} to {}x{})",
                max_x - min_x + 1,
                max_y - min_y + 1,
                min_x,
                min_y,
                max_x,
                max_y
            );

            // Show selection info at the bottom of the grid
            let info_area = Rect {
                x: grid_area.x,
                y: grid_area.y + grid_area.height.saturating_sub(1),
                width: grid_area.width,
                height: 1,
            };

            let info_paragraph = Paragraph::new(selection_info)
                .style(Style::default().fg(Color::Yellow).bg(Color::Blue));

            f.render_widget(info_paragraph, info_area);
        }
    }

    fn render_stats(&self, f: &mut Frame, area: Rect) {
        let block = Block::default().title("Statistics").borders(Borders::ALL);

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Calculate flag statistics
        let mut flag_counts: HashMap<&str, u32> = HashMap::new();
        let mut category_counts: HashMap<FlagCategory, u32> = HashMap::new();
        let mut total_pages = 0;
        let mut pages_with_flags = 0;

        for page in &self.state.pages {
            total_pages += 1;
            if page.flags != 0 {
                pages_with_flags += 1;

                // Count individual flags
                for (flag, name, _, category) in PAGE_FLAGS {
                    if page.flags & flag != 0 {
                        *flag_counts.entry(name).or_insert(0) += 1;
                        *category_counts.entry(*category).or_insert(0) += 1;
                    }
                }
            }
        }

        // Create stats text
        let mut stats_lines = vec![
            Line::from(vec![
                Span::styled("Total Pages: ", Style::default().fg(Color::Yellow)),
                Span::styled(total_pages.to_string(), Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("With Flags: ", Style::default().fg(Color::Green)),
                Span::styled(
                    pages_with_flags.to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Top Flags:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        // Add top flags
        let mut sorted_flags: Vec<_> = flag_counts.iter().collect();
        sorted_flags.sort_by(|a, b| b.1.cmp(a.1));

        for (flag, count) in sorted_flags.iter().take(8) {
            let percentage = if total_pages > 0 {
                (**count as f64 / total_pages as f64) * 100.0
            } else {
                0.0
            };

            stats_lines.push(Line::from(vec![
                Span::styled(format!("{}: ", flag), Style::default().fg(Color::Green)),
                Span::styled(
                    format!("{} ({:.1}%)", count, percentage),
                    Style::default().fg(Color::White),
                ),
            ]));
        }

        stats_lines.push(Line::from(""));
        stats_lines.push(Line::from(Span::styled(
            "Categories:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));

        // Add category stats
        let mut sorted_categories: Vec<_> = category_counts.iter().collect();
        sorted_categories.sort_by(|a, b| b.1.cmp(a.1));

        for (category, count) in sorted_categories.iter() {
            let percentage = if total_pages > 0 {
                (**count as f64 / total_pages as f64) * 100.0
            } else {
                0.0
            };

            let (symbol, color) = get_category_symbol_and_color(**category);
            stats_lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", symbol),
                    Style::default().fg(self.ratatui_color_from_colored(color)),
                ),
                Span::styled(
                    format!("{:?}: ", category),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{} ({:.1}%)", count, percentage),
                    Style::default().fg(Color::White),
                ),
            ]));
        }

        let stats_text = Text::from(stats_lines);
        let stats_paragraph = Paragraph::new(stats_text).wrap(Wrap { trim: false });

        f.render_widget(stats_paragraph, inner);
    }

    fn render_help(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(Span::styled(
                "KPageFlags TUI Help",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  Arrow Keys    - Move around the grid"),
            Line::from("  +/=           - Zoom in"),
            Line::from("  -             - Zoom out"),
            Line::from("  Home          - Reset view to origin"),
            Line::from(""),
            Line::from("Mouse Controls:"),
            Line::from("  Click & Drag  - Select area to zoom into"),
            Line::from("  Scroll Up     - Zoom in"),
            Line::from("  Scroll Down   - Zoom out"),
            Line::from("  Esc           - Cancel selection"),
            Line::from(""),
            Line::from("Controls:"),
            Line::from("  h             - Toggle this help"),
            Line::from("  s             - Toggle statistics panel"),
            Line::from("  r             - Refresh data"),
            Line::from("  q             - Quit"),
            Line::from(""),
            Line::from("Filters (show only pages with these flag categories):"),
            Line::from("  1             - State flags (LOCKED, DIRTY, etc.)"),
            Line::from("  2             - Memory management (LRU, ACTIVE, etc.)"),
            Line::from("  3             - Usage tracking (REFERENCED, ANON, etc.)"),
            Line::from("  4             - Allocation (BUDDY, SLAB)"),
            Line::from("  5             - I/O related (WRITEBACK)"),
            Line::from("  6             - Structure (HUGE, THP, etc.)"),
            Line::from("  7             - Special (KSM, ZERO_PAGE, etc.)"),
            Line::from("  8             - Error flags (ERROR, HWPOISON)"),
            Line::from("  0             - Clear filter (show all)"),
            Line::from(""),
            Line::from("Grid Symbols:"),
            Line::from("  S - State flags      M - Memory mgmt      U - Usage tracking"),
            Line::from("  A - Allocation       I - I/O related      T - Structure"),
            Line::from("  P - Special          E - Error flags      . - No flags"),
        ];

        let help_paragraph = Paragraph::new(Text::from(help_text))
            .block(Block::default().title("Help").borders(Borders::ALL))
            .wrap(Wrap { trim: false });

        f.render_widget(help_paragraph, area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let filter_text = if let Some(cat) = self.state.filter_category {
            format!("Filter: {:?}", cat)
        } else {
            "Filter: None".to_string()
        };

        let selection_text = if self.state.mouse_selecting {
            " | Selecting..."
        } else {
            ""
        };

        let footer_text = format!(
            "Press 'h' for help | 'q' to quit | {} | Offset: ({}, {}) | Zoom: {:.1}x{}",
            filter_text,
            self.state.offset_x,
            self.state.offset_y,
            self.state.zoom_level,
            selection_text
        );

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, area);
    }

    fn get_page_symbol_and_color(&self, page: &PageInfo) -> (char, Color) {
        if page.flags == 0 {
            return ('.', Color::DarkGray);
        }

        let categories = page.get_flag_categories();
        if categories.len() == 1 {
            let (symbol_char, colored_color) = get_category_symbol_and_color(categories[0]);
            (symbol_char, self.ratatui_color_from_colored(colored_color))
        } else if categories.len() > 1 {
            ('●', Color::White)
        } else {
            ('?', Color::Red)
        }
    }

    fn ratatui_color_from_colored(&self, color: colored::Color) -> Color {
        match color {
            colored::Color::Blue => Color::Blue,
            colored::Color::Green => Color::Green,
            colored::Color::Yellow => Color::Yellow,
            colored::Color::Cyan => Color::Cyan,
            colored::Color::Magenta => Color::Magenta,
            colored::Color::Red => Color::Red,
            colored::Color::White => Color::White,
            colored::Color::BrightRed => Color::LightRed,
            colored::Color::BrightWhite => Color::White,
            _ => Color::White,
        }
    }
}

pub async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let mut app = TuiApp::new()?;
    let res = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}
