use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

fn main() -> io::Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

struct App {
    input: String,
    cursor: usize,
    output: Vec<String>,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            output: vec![],
        }
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    fn newline(&mut self) {
        self.input.push('\n');
        self.cursor = self.input.len();
    }

    fn submit(&mut self) {
        let code = self.input.trim().to_string();
        if !code.is_empty() {
            self.output.push(format!("> {}", code));

            // 👉 replace this with your interpreter
            self.output.push(format!("result: {}", fake_run(&code)));
        }

        self.input.clear();
        self.cursor = 0;
    }
}

fn fake_run(input: &str) -> String {
    format!("executed ({})", input)
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(f.area());

            // OUTPUT PANEL
            let output: Vec<Line> = app.output.iter().map(|l| Line::from(l.clone())).collect();

            let output_widget = Paragraph::new(output)
                .block(Block::default().title("Output").borders(Borders::ALL));

            // INPUT PANEL (with naive highlighting)
            let highlighted = highlight(&app.input);

            let input_widget = Paragraph::new(highlighted)
                .block(Block::default().title("Input").borders(Borders::ALL));

            f.render_widget(output_widget, chunks[0]);
            f.render_widget(input_widget, chunks[1]);
        })?;

        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Char(c) => app.insert_char(c),
                KeyCode::Backspace => app.backspace(),
                KeyCode::Enter => app.submit(),
                KeyCode::Esc => return Ok(()),
                KeyCode::Tab => app.insert_char('\t'),
                KeyCode::Down => app.newline(), // simple multiline shortcut
                _ => {}
            }
        }
    }
}

fn highlight(input: &str) -> Vec<Line> {
    input
        .split_whitespace()
        .map(|word| {
            let style = match word {
                "fun" | "var" | "if" => Style::default().fg(Color::Blue),
                "print" => Style::default().fg(Color::Green),
                _ => Style::default().fg(Color::White),
            };

            Line::from(Span::styled(format!("{word} "), style))
        })
        .collect()
}
