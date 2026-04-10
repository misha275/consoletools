use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{poll, read, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
    LeaveAlternateScreen,
};

/// ANSI reset.
pub const NORMAL: &str = "\x1b[0m";

/// Fixed base log color.
pub const BASE_LOG_COLOR: &str = "\x1b[37m";

/// Fixed base success color.
pub const BASE_OK_COLOR: &str = "\x1b[32m";

/// Fixed base error color.
pub const BASE_ERR_COLOR: &str = "\x1b[31m";

/// Three fixed base colors available to all output formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaseColor {
    Log,
    Ok,
    Err,
}

impl BaseColor {
    fn code(self) -> &'static str {
        match self {
            BaseColor::Log => "37",
            BaseColor::Ok => "32",
            BaseColor::Err => "31",
        }
    }
}

/// Extended color palette for custom formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl CustomColor {
    fn code(self) -> &'static str {
        match self {
            CustomColor::Black => "30",
            CustomColor::Red => "31",
            CustomColor::Green => "32",
            CustomColor::Yellow => "33",
            CustomColor::Blue => "34",
            CustomColor::Magenta => "35",
            CustomColor::Cyan => "36",
            CustomColor::White => "37",
            CustomColor::BrightBlack => "90",
            CustomColor::BrightRed => "91",
            CustomColor::BrightGreen => "92",
            CustomColor::BrightYellow => "93",
            CustomColor::BrightBlue => "94",
            CustomColor::BrightMagenta => "95",
            CustomColor::BrightCyan => "96",
            CustomColor::BrightWhite => "97",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextColor {
    Base(BaseColor),
    Custom(CustomColor),
}

/// An output format made of a color and optional text styles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextFormat {
    color: TextColor,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
    pub underline: bool,
    pub reverse: bool,
    pub strike: bool,
}

impl TextFormat {
    /// Create a new format using one of the three fixed base colors.
    pub fn new(base: BaseColor) -> Self {
        Self {
            color: TextColor::Base(base),
            bold: false,
            italic: false,
            dim: false,
            underline: false,
            reverse: false,
            strike: false,
        }
    }

    /// Create a custom format using the extended palette.
    pub fn custom(color: CustomColor) -> Self {
        Self {
            color: TextColor::Custom(color),
            bold: false,
            italic: false,
            dim: false,
            underline: false,
            reverse: false,
            strike: false,
        }
    }

    /// Enable or disable bold.
    pub fn bold(mut self, enabled: bool) -> Self {
        self.bold = enabled;
        self
    }

    /// Enable or disable italic.
    pub fn italic(mut self, enabled: bool) -> Self {
        self.italic = enabled;
        self
    }

    /// Enable or disable dim.
    pub fn dim(mut self, enabled: bool) -> Self {
        self.dim = enabled;
        self
    }

    /// Enable or disable underline.
    pub fn underline(mut self, enabled: bool) -> Self {
        self.underline = enabled;
        self
    }

    /// Enable or disable reverse.
    pub fn reverse(mut self, enabled: bool) -> Self {
        self.reverse = enabled;
        self
    }

    /// Enable or disable strike-through.
    pub fn strike(mut self, enabled: bool) -> Self {
        self.strike = enabled;
        self
    }

    fn ansi_prefix(&self) -> String {
        let mut codes: Vec<&str> = vec![match self.color {
            TextColor::Base(color) => color.code(),
            TextColor::Custom(color) => color.code(),
        }];
        if self.bold {
            codes.push("1");
        }
        if self.dim {
            codes.push("2");
        }
        if self.italic {
            codes.push("3");
        }
        if self.underline {
            codes.push("4");
        }
        if self.reverse {
            codes.push("7");
        }
        if self.strike {
            codes.push("9");
        }

        format!("\x1b[{}m", codes.join(";"))
    }

    /// Apply the format to a string.
    pub fn apply(&self, text: &str) -> String {
        format!("{}{}{}", self.ansi_prefix(), text, NORMAL)
    }
}

impl Default for TextFormat {
    fn default() -> Self {
        Self::new(BaseColor::Log)
    }
}

static GLOBAL_FORMATS: OnceLock<Mutex<HashMap<String, TextFormat>>> = OnceLock::new();
static GLOBAL_CONSOLE_HANDLE: OnceLock<ConsoleHandle> = OnceLock::new();
static GLOBAL_PRINT_BUFFER: OnceLock<Mutex<String>> = OnceLock::new();

fn formats_store() -> &'static Mutex<HashMap<String, TextFormat>> {
    GLOBAL_FORMATS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn resolve_format(name: &str) -> TextFormat {
    match name {
        "log" => TextFormat::new(BaseColor::Log),
        "ok" => TextFormat::new(BaseColor::Ok),
        "err" => TextFormat::new(BaseColor::Err),
        "prompt" => formats_store()
            .lock()
            .ok()
            .and_then(|formats| formats.get(name).cloned())
            .unwrap_or_else(|| TextFormat::new(BaseColor::Log).bold(true)),
        _ => formats_store()
            .lock()
            .ok()
            .and_then(|formats| formats.get(name).cloned())
            .unwrap_or_else(|| TextFormat::new(BaseColor::Log)),
    }
}

/// Register or replace a custom output format by name.
///
/// Returns `false` if the name is reserved for fixed base outputs.
pub fn register_output_format(name: impl Into<String>, format: TextFormat) -> bool {
    let name = name.into();
    if matches!(name.as_str(), "log" | "ok" | "err") {
        return false;
    }

    if let Ok(mut formats) = formats_store().lock() {
        formats.insert(name, format);
        return true;
    }

    false
}

/// Format a string with one of the registered output formats.
pub fn format_output(name: &str, text: impl Into<String>) -> String {
    resolve_format(name).apply(&text.into())
}

fn base_output(color: BaseColor, text: impl Into<String>) -> String {
    TextFormat::new(color).apply(&text.into())
}

/// Result of command handler execution.
pub enum CommandOutput {
    /// Neutral log line.
    Info(String),
    /// Success log line.
    Success(String),
    /// Error log line.
    Error(String),
}

enum ConsoleEvent {
    Line(String),
}

#[derive(Clone)]
/// Thread-safe handle used to push log lines into the console from any thread.
pub struct ConsoleHandle {
    sender: Sender<ConsoleEvent>,
}

impl ConsoleHandle {
    /// Send a neutral message to the console.
    pub fn info(&self, message: impl Into<String>) {
        let _ = self.sender.send(ConsoleEvent::Line(base_output(BaseColor::Log, message)));
    }

    /// Send a success message to the console.
    pub fn success(&self, message: impl Into<String>) {
        let _ = self.sender.send(ConsoleEvent::Line(base_output(BaseColor::Ok, message)));
    }

    /// Send an error message to the console.
    pub fn error(&self, message: impl Into<String>) {
        let _ = self.sender.send(ConsoleEvent::Line(base_output(BaseColor::Err, message)));
    }
}

/// Install global output handle for console macros.
///
/// Call this once before using cprint/cprintln or custom format macros.
pub fn install_global_console_handle(handle: ConsoleHandle) {
    let _ = GLOBAL_CONSOLE_HANDLE.set(handle);
    let _ = GLOBAL_PRINT_BUFFER.set(Mutex::new(String::new()));
    let _ = formats_store();
}

fn send_rendered_line(line: String) {
    if let Some(handle) = GLOBAL_CONSOLE_HANDLE.get() {
        let _ = handle.sender.send(ConsoleEvent::Line(line));
    }
}

/// Low-level text writer used by cprint/cprintln.
pub fn console_write_raw(text: &str) {
    if let Some(buffer_lock) = GLOBAL_PRINT_BUFFER.get() {
        let mut buffer = match buffer_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        buffer.push_str(text);

        loop {
            let Some(newline_index) = buffer.find('\n') else {
                break;
            };

            let mut line = buffer.drain(..=newline_index).collect::<String>();
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }

            send_rendered_line(base_output(BaseColor::Log, line));
        }
        return;
    }

    send_rendered_line(base_output(BaseColor::Log, text));
}

/// Send a neutral log line to the console.
pub fn console_write_log(text: impl Into<String>) {
    send_rendered_line(base_output(BaseColor::Log, text));
}

/// Send a success line to the console.
pub fn console_write_ok(text: impl Into<String>) {
    send_rendered_line(base_output(BaseColor::Ok, text));
}

/// Send an error line to the console.
pub fn console_write_err(text: impl Into<String>) {
    send_rendered_line(base_output(BaseColor::Err, text));
}

/// Send a line using any registered format.
pub fn console_write_format(format_name: &str, text: impl Into<String>) {
    send_rendered_line(format_output(format_name, text));
}

#[macro_export]
macro_rules! cprint {
    ($($arg:tt)*) => {{
        $crate::core::console_write_raw(&format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cprintln {
    () => {{
        $crate::core::console_write_log("");
    }};
    ($($arg:tt)*) => {{
        $crate::core::console_write_log(format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cprintln_log {
    () => {{
        $crate::core::console_write_log("");
    }};
    ($($arg:tt)*) => {{
        $crate::core::console_write_log(format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cprintln_ok {
    () => {{
        $crate::core::console_write_ok("");
    }};
    ($($arg:tt)*) => {{
        $crate::core::console_write_ok(format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cprintln_err {
    () => {{
        $crate::core::console_write_err("");
    }};
    ($($arg:tt)*) => {{
        $crate::core::console_write_err(format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! cprintln_fmt {
    ($format_name:expr, $($arg:tt)*) => {{
        $crate::core::console_write_format($format_name, format!($($arg)*));
    }};
    ($format_name:expr) => {{
        $crate::core::console_write_format($format_name, "");
    }};
}

struct RegisteredCommand {
    description: String,
    handler: Box<dyn Fn(&[String]) -> CommandOutput + Send + Sync>,
}

pub struct CommandConsole {
    prompt: String,
    input: String,
    logs: Vec<String>,
    autosave_path: Option<PathBuf>,
    commands: HashMap<String, RegisteredCommand>,
    event_sender: Sender<ConsoleEvent>,
    event_receiver: Receiver<ConsoleEvent>,
}

impl CommandConsole {
    /// Create interactive console with prompt, for example ">>".
    pub fn new(prompt: &str) -> Self {
        let (event_sender, event_receiver) = mpsc::channel();
        let _ = formats_store();

        Self {
            prompt: prompt.to_string(),
            input: String::new(),
            logs: Vec::new(),
            autosave_path: None,
            commands: HashMap::new(),
            event_sender,
            event_receiver,
        }
    }

    /// Get a handle that can be used from other threads.
    pub fn handle(&self) -> ConsoleHandle {
        ConsoleHandle {
            sender: self.event_sender.clone(),
        }
    }

    /// Register a command handler.
    pub fn add_command<F>(&mut self, name: &str, description: &str, handler: F)
    where
        F: Fn(&[String]) -> CommandOutput + Send + Sync + 'static,
    {
        self.commands.insert(
            name.to_string(),
            RegisteredCommand {
                description: description.to_string(),
                handler: Box::new(handler),
            },
        );
    }

    /// Start interactive event loop.
    pub fn run(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, Hide)?;

        self.push_info("Интерактивная консоль запущена. help - список команд, exit - выход.");
        self.render(&mut stdout)?;

        let mut should_continue = true;
        while should_continue {
            let mut should_render = false;

            while let Ok(event) = self.event_receiver.try_recv() {
                self.apply_event(event);
                should_render = true;
            }

            if poll(Duration::from_millis(60))? {
                if let Event::Key(key) = read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.push_error("Остановлено пользователем (Ctrl+C)");
                            should_continue = false;
                        }
                        KeyCode::Char(ch) => self.input.push(ch),
                        KeyCode::Backspace => {
                            self.input.pop();
                        }
                        KeyCode::Esc => {
                            self.input.clear();
                        }
                        KeyCode::Enter => {
                            let command_line = self.input.trim().to_string();
                            self.input.clear();
                            should_continue = self.execute_command_line(&command_line);
                        }
                        _ => {}
                    }

                    should_render = true;
                }
            }

            if should_render {
                self.render(&mut stdout)?;
            }
        }

        execute!(stdout, Show, LeaveAlternateScreen)?;
        disable_raw_mode()
    }

    /// Save current console log to a file.
    ///
    /// ANSI escape sequences are stripped from output for clean text logs.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let mut content = String::new();
        for line in &self.logs {
            content.push_str(&strip_ansi(line));
            content.push('\n');
        }

        fs::write(path, content)
    }

    /// Enable dynamic autosave for every new log line.
    ///
    /// Existing log is saved immediately, then every next output line is appended.
    pub fn enable_autosave<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let path_buf = path.as_ref().to_path_buf();
        self.save_to_file(&path_buf)?;
        self.autosave_path = Some(path_buf);
        Ok(())
    }

    /// Disable dynamic autosave.
    pub fn disable_autosave(&mut self) {
        self.autosave_path = None;
    }

    fn apply_event(&mut self, event: ConsoleEvent) {
        match event {
            ConsoleEvent::Line(line) => self.append_log_line(line),
        }
    }

    fn execute_command_line(&mut self, command_line: &str) -> bool {
        if command_line.is_empty() {
            return true;
        }

        self.logs.push(format_output(
            "prompt",
            &format!("{} {}", self.prompt, command_line),
        ));

        let mut parts = command_line.split_whitespace();
        let Some(command) = parts.next() else {
            return true;
        };
        let args: Vec<String> = parts.map(ToString::to_string).collect();

        match command {
            "exit" | "quit" => {
                self.push_info("Выход из консоли");
                false
            }
            "help" => {
                self.push_info("Доступные команды:");
                let mut names: Vec<String> = self.commands.keys().cloned().collect();
                names.sort();

                for name in names {
                    if let Some(cmd) = self.commands.get(&name) {
                        self.push_info(&format!(
                            "- {}: {}",
                            name, cmd.description
                        ));
                    }
                }
                self.push_info("- help: показать список команд");
                self.push_info("- clear: очистить экран вывода");
                self.push_info("- save <путь/имя_файла>: сохранить лог консоли в файл");
                self.push_info("- autosave on <путь/имя_файла>: включить динамическое сохранение");
                self.push_info("- autosave off: выключить динамическое сохранение");
                self.push_info("- exit/quit: выйти");
                true
            }
            "clear" => {
                self.logs.clear();
                true
            }
            "save" => {
                if args.is_empty() {
                    self.push_error("Использование: save <путь/имя_файла>");
                    return true;
                }

                let path = args.join(" ");
                match self.save_to_file(&path) {
                    Ok(_) => self.push_success(&format!("Лог сохранен в: {}", path)),
                    Err(err) => self.push_error(&format!("Ошибка сохранения: {}", err)),
                }
                true
            }
            "autosave" => {
                let Some(mode) = args.first().map(String::as_str) else {
                    self.push_error("Использование: autosave on <путь/имя_файла> | autosave off");
                    return true;
                };

                match mode {
                    "on" => {
                        if args.len() < 2 {
                            self.push_error("Использование: autosave on <путь/имя_файла>");
                            return true;
                        }

                        let path = args[1..].join(" ");
                        match self.enable_autosave(&path) {
                            Ok(_) => self.push_success(&format!("Динамическое сохранение включено: {}", path)),
                            Err(err) => self.push_error(&format!("Ошибка включения autosave: {}", err)),
                        }
                        true
                    }
                    "off" => {
                        self.disable_autosave();
                        self.push_success("Динамическое сохранение выключено");
                        true
                    }
                    _ => {
                        self.push_error("Использование: autosave on <путь/имя_файла> | autosave off");
                        true
                    }
                }
            }
            _ => {
                if let Some(cmd) = self.commands.get(command) {
                    match (cmd.handler)(&args) {
                        CommandOutput::Info(msg) => self.push_info(&msg),
                        CommandOutput::Success(msg) => self.push_success(&msg),
                        CommandOutput::Error(msg) => self.push_error(&msg),
                    }
                } else {
                    self.push_error(&format!("Неизвестная команда: {}", command));
                }
                true
            }
        }
    }

    fn render(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        let (width, height) = size()?;
        let max_log_lines = height.saturating_sub(1) as usize;
        let first_line = self.logs.len().saturating_sub(max_log_lines);

        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        for line in &self.logs[first_line..] {
            writeln!(stdout, "{}", line)?;
        }

        let prompt_format = resolve_format("prompt");
        let prompt_text = format!("{} ", prompt_format.apply(&self.prompt));
        let prompt_width = self.prompt.chars().count() + 1;
        let max_input_chars = (width as usize).saturating_sub(prompt_width);
        let visible_input = Self::tail_chars(&self.input, max_input_chars);

        execute!(
            stdout,
            MoveTo(0, height.saturating_sub(1)),
            Clear(ClearType::CurrentLine)
        )?;
        write!(stdout, "{}{}", prompt_text, visible_input)?;
        stdout.flush()
    }

    fn tail_chars(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }

        s.chars()
            .rev()
            .take(max_chars)
            .collect::<Vec<char>>()
            .into_iter()
            .rev()
            .collect()
    }

    fn push_info(&mut self, text: &str) {
        self.append_log_line(base_output(BaseColor::Log, text));
    }

    fn push_success(&mut self, text: &str) {
        self.append_log_line(base_output(BaseColor::Ok, text));
    }

    fn push_error(&mut self, text: &str) {
        self.append_log_line(base_output(BaseColor::Err, text));
    }

    fn append_log_line(&mut self, rendered_line: String) {
        if let Some(path) = &self.autosave_path {
            let plain = strip_ansi(&rendered_line);
            if let Err(err) = append_line_to_file(path, &plain) {
                self.logs.push(base_output(
                    BaseColor::Err,
                    format!("Ошибка autosave: {}", err),
                ));
            }
        }

        self.logs.push(rendered_line);
    }
}

/// Format helper for informational output.
pub fn color_fmt_ok(s: &str, vars: &[&str]) -> String {
    color_fmt_impl(TextFormat::new(BaseColor::Ok).bold(true), s, vars)
}

/// Format helper for error output.
pub fn color_fmt_err(s: &str, vars: &[&str]) -> String {
    color_fmt_impl(TextFormat::new(BaseColor::Err).bold(true), s, vars)
}

/// Format helper for plain log output.
pub fn color_fmt_log(s: &str, vars: &[&str]) -> String {
    color_fmt_impl(TextFormat::new(BaseColor::Log).bold(true), s, vars)
}

fn color_fmt_impl(format: TextFormat, s: &str, vars: &[&str]) -> String {
    let mut out = String::new();
    let mut parts = s.split("{}");

    for (i, part) in parts.by_ref().enumerate() {
        out.push_str(&format.apply(part));
        if i < vars.len() {
            out.push_str(&TextFormat::new(BaseColor::Log).dim(true).apply(vars[i]));
        }
    }

    out
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
        }
        out.push(ch);
    }

    out
}

fn append_line_to_file(path: &Path, line: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)
}
