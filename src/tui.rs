//! Two-pane ratatui UI. `d` marks paths; files are never deleted.

use crate::scan::{format_bytes, scan_dir, DuplicateGroup};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use std::collections::BTreeSet;
use std::io::{self, Stdout};
use std::path::Path;
use std::time::Duration;

const BG: Color = Color::Rgb(13, 17, 23);
const BORDER: Color = Color::Rgb(88, 166, 255);
const TITLE: Color = Color::Rgb(63, 185, 80);
const TEXT: Color = Color::Rgb(201, 209, 216);
const MUTED: Color = Color::Rgb(139, 148, 158);
const WASTE: Color = Color::Rgb(210, 153, 34);
const MARK: Color = Color::Rgb(248, 81, 73);
const SEL_BG: Color = Color::Rgb(31, 111, 235);

struct App {
    groups: Vec<DuplicateGroup>,
    group_state: ListState,
    file_state: ListState,
    focus_files: bool,
    marked: BTreeSet<String>,
    root: String,
    wasted: u64,
}

impl App {
    fn new(groups: Vec<DuplicateGroup>, root: String, wasted: u64) -> Self {
        let mut group_state = ListState::default();
        if !groups.is_empty() {
            group_state.select(Some(0));
        }
        let mut file_state = ListState::default();
        if groups.first().is_some_and(|g| !g.paths.is_empty()) {
            file_state.select(Some(0));
        }
        Self {
            groups,
            group_state,
            file_state,
            focus_files: false,
            marked: BTreeSet::new(),
            root,
            wasted,
        }
    }

    fn group_idx(&self) -> Option<usize> {
        self.group_state.selected()
    }

    fn current_group(&self) -> Option<&DuplicateGroup> {
        self.group_idx().and_then(|i| self.groups.get(i))
    }

    fn reset_file_cursor(&mut self) {
        let n = self.current_group().map(|g| g.paths.len()).unwrap_or(0);
        self.file_state.select(if n == 0 { None } else { Some(0) });
    }

    fn move_group(&mut self, delta: isize) {
        let n = self.groups.len();
        if n == 0 {
            return;
        }
        let i = self.group_idx().unwrap_or(0);
        let next = (i as isize + delta).rem_euclid(n as isize) as usize;
        self.group_state.select(Some(next));
        self.reset_file_cursor();
    }

    fn move_file(&mut self, delta: isize) {
        let n = self.current_group().map(|g| g.paths.len()).unwrap_or(0);
        if n == 0 {
            return;
        }
        let i = self.file_state.selected().unwrap_or(0);
        let next = (i as isize + delta).rem_euclid(n as isize) as usize;
        self.file_state.select(Some(next));
    }

    fn down(&mut self) {
        if self.focus_files {
            self.move_file(1);
        } else {
            self.move_group(1);
        }
    }

    fn up(&mut self) {
        if self.focus_files {
            self.move_file(-1);
        } else {
            self.move_group(-1);
        }
    }

    fn toggle_expand(&mut self) {
        if self.groups.is_empty() {
            return;
        }
        self.focus_files = !self.focus_files;
        if self.focus_files && self.file_state.selected().is_none() {
            self.reset_file_cursor();
        }
    }

    fn mark(&mut self) {
        let Some(g) = self.current_group().cloned() else {
            return;
        };
        if self.focus_files {
            if let Some(i) = self.file_state.selected() {
                if let Some(p) = g.paths.get(i) {
                    toggle_mark(&mut self.marked, p);
                }
            }
        } else {
            for p in &g.paths {
                toggle_mark(&mut self.marked, p);
            }
        }
    }
}

fn toggle_mark(set: &mut BTreeSet<String>, path: &str) {
    if !set.remove(path) {
        set.insert(path.to_string());
    }
}

pub fn run(root: &Path, min_bytes: u64) -> Result<()> {
    let report = scan_dir(root, min_bytes)?;
    let app = App::new(report.groups, report.root.clone(), report.wasted_bytes);
    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, app);
    let _ = restore_terminal(&mut terminal);
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, &mut app))?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('j') | KeyCode::Down => app.down(),
            KeyCode::Char('k') | KeyCode::Up => app.up(),
            KeyCode::Enter | KeyCode::Right => {
                if !app.focus_files {
                    app.toggle_expand();
                }
            }
            KeyCode::Left => {
                if app.focus_files {
                    app.focus_files = false;
                }
            }
            KeyCode::Char('d') | KeyCode::Char(' ') => app.mark(),
            _ => {}
        }
    }

    if !app.marked.is_empty() {
        println!("marked (not deleted):");
        for p in &app.marked {
            println!("  {p}");
        }
    }
    Ok(())
}

fn draw(f: &mut ratatui::Frame, app: &mut App) {
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(TEXT)),
        f.area(),
    );

    let cols = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(f.area());

    let header = Line::from(vec![
        Span::styled(
            " twinseek ",
            Style::default().fg(TITLE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(app.root.clone(), Style::default().fg(TEXT)),
        Span::styled(
            format!(
                "  ·  {} groups  ·  {} wasted",
                app.groups.len(),
                format_bytes(app.wasted)
            ),
            Style::default().fg(MUTED),
        ),
    ]);
    f.render_widget(Paragraph::new(header), cols[0]);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(cols[1]);

    let group_items: Vec<ListItem> = if app.groups.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            " no duplicates",
            Style::default().fg(MUTED),
        )))]
    } else {
        app.groups
            .iter()
            .map(|g| {
                let label = g.paths.first().map(|p| p.as_str()).unwrap_or("?");
                let short = label.rsplit('/').next().unwrap_or(label);
                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {:>2}×  ", g.count), Style::default().fg(TITLE)),
                    Span::styled(
                        format!("{:>8} wasted  ", format_bytes(g.wasted_bytes)),
                        Style::default().fg(WASTE),
                    ),
                    Span::styled(short.to_string(), Style::default().fg(TEXT)),
                ]))
            })
            .collect()
    };

    let left_title = if app.focus_files {
        " groups "
    } else {
        " groups ● "
    };
    let left = List::new(group_items)
        .block(
            Block::default()
                .title(left_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if app.focus_files { MUTED } else { BORDER }))
                .style(Style::default().bg(BG)),
        )
        .highlight_style(
            Style::default()
                .bg(SEL_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");
    f.render_stateful_widget(left, panes[0], &mut app.group_state);

    let file_items: Vec<ListItem> = match app.current_group() {
        None => vec![ListItem::new(Line::from(Span::styled(
            " (select a group)",
            Style::default().fg(MUTED),
        )))],
        Some(g) => g
            .paths
            .iter()
            .map(|p| {
                let flagged = app.marked.contains(p);
                let mark = if flagged { "* " } else { "  " };
                let style = if flagged {
                    Style::default().fg(MARK)
                } else {
                    Style::default().fg(TEXT)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(mark, Style::default().fg(MARK)),
                    Span::styled(p.clone(), style),
                ]))
            })
            .collect(),
    };

    let right_title = if app.focus_files {
        " files ●  * = marked "
    } else {
        " files "
    };
    let right = List::new(file_items)
        .block(
            Block::default()
                .title(right_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if app.focus_files { BORDER } else { MUTED }))
                .style(Style::default().bg(BG)),
        )
        .highlight_style(
            Style::default()
                .bg(SEL_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");
    f.render_stateful_widget(right, panes[1], &mut app.file_state);

    let footer = Line::from(vec![
        Span::styled(
            " j/k ",
            Style::default().fg(TITLE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("move  ", Style::default().fg(MUTED)),
        Span::styled(
            "enter ",
            Style::default().fg(TITLE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("expand  ", Style::default().fg(MUTED)),
        Span::styled(
            "d ",
            Style::default().fg(TITLE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("mark  ", Style::default().fg(MUTED)),
        Span::styled(
            "q ",
            Style::default().fg(TITLE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("quit  ", Style::default().fg(MUTED)),
        Span::styled(
            format!(" marked {}  ·  never deletes", app.marked.len()),
            Style::default().fg(WASTE),
        ),
    ]);
    f.render_widget(Paragraph::new(footer), cols[2]);
}
