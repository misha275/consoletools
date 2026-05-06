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

/// Creates a custom text format with specified colors and text styling options.
///
/// # Arguments
///
/// * `text_color` - The color to apply to the main text (from `CustomColor` enum)
/// * `text_formatting` - A closure that configures text styling (bold, italic, underline, etc.)
/// * `param_color` - The color for parameter/placeholder text (from `CustomColor` enum)
/// * `param_formatting` - A closure that configures styling for parameter text
///
/// # Returns
///
/// A `TextFormat` struct that can be registered and used for console output.
///
/// # Example
///
/// ```ignore
/// let format = create_format(
///     CustomColor::BrightGreen,
///     |f| f.bold(true),
///     CustomColor::Green,
///     |f| f.dim(true)
/// );
/// register_output_format("custom", format);
/// ```
pub fn create_format<F1, F2>(
    text_color: CustomColor,
    text_formatting: F1,
    param_color: CustomColor,
    param_formatting: F2,
) -> TextFormat
where
    F1: FnOnce(TextFormat) -> TextFormat,
    F2: FnOnce(TextFormat) -> TextFormat,
{
    let _param_format = param_formatting(TextFormat::custom(param_color));
    text_formatting(TextFormat::custom(text_color))
}

/// Register or replace a custom output format by name.
///
/// Custom formats can be used with `cprintln_format()` or other format-aware functions.
/// This function allows dynamic registration of named formats at runtime.
///
/// # Arguments
///
/// * `name` - The name identifier for this format (cannot be "log", "ok", or "err" - these are reserved)
/// * `format` - The `TextFormat` structure defining color and styling
///
/// # Returns
///
/// Returns `true` if the format was successfully registered, `false` if the name is reserved.
///
/// # Example
///
/// ```ignore
/// let format = TextFormat::custom(CustomColor::BrightBlue).bold(true);
/// register_output_format("warning", format);
/// ```
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

/// Formats text with one of the registered output formats.
///
/// This is a low-level function used internally by the macro system.
/// For most use cases, prefer using the macro equivalents like `cprintln_format!()`.
///
/// # Arguments
///
/// * `name` - The format name to apply ("log", "ok", "err", or a custom registered format)
/// * `text` - The text to format
///
/// # Returns
///
/// A formatted string with ANSI color codes applied.
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
/// Thread-safe handle for sending log lines to the console from any thread.
///
/// `ConsoleHandle` can be cloned and shared across threads, allowing safe
/// non-blocking communication with the main console. Messages are queued and
/// processed by the console's event loop.
///
/// # Usage
///
/// Obtain a handle from `CommandConsole::handle()`, then use the `send()` method with formatting helpers:
///
/// ```ignore
/// use consoletools::{format_ok, format_err, format_log};
///
/// let handle = console.handle();
/// std::thread::spawn(move || {
///     handle.send(format_log("Message from thread"));
///     handle.send(format_ok("Operation completed"));
///     handle.send(format_err("Error occurred"));
/// });
/// ```
pub struct ConsoleHandle {
    sender: Sender<ConsoleEvent>,
}

impl ConsoleHandle {
    /// Send a message to the console.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send. Use helper functions like `format_ok()`, `format_err()`
    ///              to apply formatting with colors and styles.
    pub fn send(&self, message: impl Into<String>) {
        let _ = self.sender.send(ConsoleEvent::Line(message.into()));
    }
}

/// Install global output handle for console macros.
///
/// This function must be called once before using `cprint!()`, `cprintln!()` or any console output macros.
/// It initializes the global message channel and format storage used by the console system.
///
/// # Arguments
///
/// * `handle` - A `ConsoleHandle` obtained from `CommandConsole::handle()` or created elsewhere
///
/// # Example
///
/// ```ignore
/// let mut console = CommandConsole::new(">>");
/// let handle = console.handle();
/// install_global_console_handle(handle);
/// ```
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

/// Low-level raw text writer for the `cprint!()` macro.
///
/// This function writes text without automatic newline, buffering lines until a newline character is encountered.
/// When a newline is found, the line is rendered with the "log" format and sent to the console.
/// This allows multi-part output on a single line using multiple `cprint!()` calls.
///
/// # Arguments
///
/// * `text` - The text to write to the console buffer
///
/// # Note
///
/// This is called automatically by the `cprint!()` macro. Direct usage is rarely needed.
///
/// # Example
///
/// ```ignore
/// console_write_raw("Loading");
/// console_write_raw(".");
/// console_write_raw(".");
/// console_write_raw(".\n"); // This triggers output as a single line: "Loading..."
/// ```
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

/// Sends a neutral/informational log line to the console.
///
/// This is the primary output function for neutral informational messages.
/// It applies the "log" format (gray color by default) and sends the line to the console buffer.
///
/// # Arguments
///
/// * `text` - The message to display
///
/// # Example
///
/// ```ignore
/// console_write_log("Application started");
/// ```
pub fn console_write_log(text: impl Into<String>) {
    send_rendered_line(base_output(BaseColor::Log, text));
}



#[macro_export]
/// The primary console output macro for unformatted text.
///
/// This macro writes text without automatic newline, buffering output until a newline character is encountered.
/// Useful for building lines incrementally: `cprint!("Loading"); cprint!("."); cprint!(".\n");`
///
/// # Example
///
/// ```ignore
/// cprint!("Hello, ");
/// cprint!("{}!", "world");
/// cprint!("\n");
/// ```
macro_rules! cprint {
    ($($arg:tt)*) => {{
        $crate::core::console_write_raw(&format!($($arg)*));
    }};
}

#[macro_export]
/// The primary console output macro for complete lines with automatic newline.
///
/// This macro outputs a complete line with automatic newline and "log" format (gray color by default).
/// For formatted output, use the provided helper functions like `cprintln_ok!()`, `cprintln_err!()`, etc.
///
/// # Example
///
/// ```ignore
/// cprintln!("Application started");
/// cprintln!("Value: {}", value);
/// ```
macro_rules! cprintln {
    () => {{
        $crate::core::console_write_log("");
    }};
    ($($arg:tt)*) => {{
        $crate::core::console_write_log(format!($($arg)*));
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
    /// Creates a new Console with the specified prompt text.
    ///
    /// The prompt will be displayed at the bottom of the console when waiting for user input.
    /// Example prompts: ">>", ">", "cmd>", "shell#", etc.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt string to display (e.g., ">>")
    ///
    /// # Returns
    ///
    /// A new `CommandConsole` instance ready for command registration and execution.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut console = CommandConsole::new(">>");
    /// console.add_command("help", "Show help", |args| {
    ///     CommandOutput::Info("Help text here".to_string())
    /// });
    /// console.run()?;
    /// ```
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

    /// Returns a cloneable handle for sending messages to this console from other threads.
    ///
    /// The handle can be passed to other threads or async tasks to send log messages
    /// back to the main console without blocking. This is useful for background operations,
    /// worker threads, or async tasks that need to report their status.
    ///
    /// # Returns
    ///
    /// A `ConsoleHandle` that can be cloned and sent across thread boundaries.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use consoletools::{format_ok, format_log};
    ///
    /// let handle = console.handle();
    /// std::thread::spawn(move || {
    ///     handle.send(format_log("Processing..."));
    ///     // ... do work ...
    ///     handle.send(format_ok("Done!"));
    /// });
    /// ```
    pub fn handle(&self) -> ConsoleHandle {
        ConsoleHandle {
            sender: self.event_sender.clone(),
        }
    }

    /// Registers a new command handler in the console.
    ///
    /// Commands can be invoked by users typing the command name and optional arguments.
    /// The handler function receives the arguments as a slice of strings and returns
    /// a `CommandOutput` indicating success, info, or error.
    ///
    /// Built-in commands: `help`, `clear`, `save`, `exit`, `quit`
    ///
    /// # Arguments
    ///
    /// * `name` - The command name (what user types to invoke it)
    /// * `description` - A brief description shown in the help text
    /// * `handler` - A function that processes the command, taking args and returning output
    ///
    /// # Example
    ///
    /// ```ignore
    /// console.add_command("greet", "Greet someone", |args| {
    ///     if args.is_empty() {
    ///         CommandOutput::Error("Usage: greet <name>".to_string())
    ///     } else {
    ///         CommandOutput::Info(format!("Hello, {}!", args[0]))
    ///     }
    /// });
    /// ```
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

    /// Starts the Console event loop.
    ///
    /// This function takes over the terminal, enabling raw mode and alternate screen.
    /// It runs the console in a full-screen mode where users can interact with registered
    /// commands, view logs, and navigate through console history.
    ///
    /// The loop continues until the user types "exit" or "quit", or presses Ctrl+C.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the console ran successfully, or an `io::Error` if terminal operations fail.
    ///
    /// # Panics
    ///
    /// Does not panic, but returns errors from underlying terminal operations.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut console = CommandConsole::new(">>");
    /// console.run()?; // Blocks until user exits
    /// ```
    pub fn run(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, Hide)?;

        self.push_info("Console started. Type 'help' for commands, 'exit' to quit.");
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
                            self.push_error("Interrupted by user (Ctrl+C)");
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

    /// Saves the entire console log to a file.
    ///
    /// All ANSI color codes are stripped from the output, producing clean plain-text logs.
    /// Parent directories are created automatically if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path where the log should be saved
    ///
    /// # Returns
    ///
    /// `Ok(())` if the save succeeded, or an `io::Error` if file operations failed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// console.save_to_file("logs/session.log")?;
    /// ```
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

    /// Enables dynamic autosave for the console log.
    ///
    /// When enabled, the existing log is saved immediately, and then every new output line
    /// is appended to the file as it's generated. This allows capturing a live transcript
    /// of console activity as it happens.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path for the autosave log
    ///
    /// # Returns
    ///
    /// `Ok(())` if autosave was enabled successfully, or an `io::Error` if initial save failed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// console.enable_autosave("logs/session.log")?;
    /// // Now every console output is automatically saved
    /// ```
    pub fn enable_autosave<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let path_buf = path.as_ref().to_path_buf();
        self.save_to_file(&path_buf)?;
        self.autosave_path = Some(path_buf);
        Ok(())
    }

    /// Disables dynamic autosave for the console.
    ///
    /// After calling this, new console output will no longer be automatically saved.
    /// Previously written logs remain in the file.
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
                self.push_info("Exiting console.");
                false
            }
            "help" => {
                self.push_info("Available commands:");
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
                self.push_info("- help: show list of commands");
                self.push_info("- clear: clear output");
                self.push_info("- save <path/filename>: save console log to file");
                self.push_info("- exit/quit: exit");
                true
            }
            "clear" => {
                self.logs.clear();
                true
            }
            "save" => {
                if args.is_empty() {
                    self.push_error("Usage: save <path/filename>");
                    return true;
                }

                let path = args.join(" ");
                match self.save_to_file(&path) {
                    Ok(_) => self.push_info(&format!("Log saved to: {}", path)),
                    Err(err) => self.push_error(&format!("Save error: {}", err)),
                }
                true
            }
            _ => {
                if let Some(cmd) = self.commands.get(command) {
                    match (cmd.handler)(&args) {
                        CommandOutput::Info(msg) => self.push_info(&msg),
                        CommandOutput::Success(msg) => self.push_success(&msg),
                        CommandOutput::Error(msg) => self.push_error(&msg),
                    }
                } else {
                    self.push_error(&format!("Unknown command: {}", command));
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

/// Formats text with the success (ok) format.
///
/// Returns a formatted string with green color and ANSI codes.
/// Use with `cprintln!()` to output formatted text.
///
/// # Example
///
/// ```ignore
/// cprintln!("{}", format_ok("Operation completed"));
/// cprintln!("{}", format_ok("Saved 42 items"));
/// ```
pub fn format_ok(text: impl Into<String>) -> String {
    base_output(BaseColor::Ok, text)
}

/// Formats text with the error format.
///
/// Returns a formatted string with red color and ANSI codes.
/// Use with `cprintln!()` to output formatted text.
///
/// # Example
///
/// ```ignore
/// cprintln!("{}", format_err("Error: file not found"));
/// cprintln!("{}", format_err("Failed to load configuration"));
/// ```
pub fn format_err(text: impl Into<String>) -> String {
    base_output(BaseColor::Err, text)
}

/// Formats text with the log (info) format.
///
/// Returns a formatted string with default color and ANSI codes.
/// Use with `cprintln!()` to output formatted text.
///
/// # Example
///
/// ```ignore
/// cprintln!("{}", format_log("Application started"));
/// cprintln!("{}", format_log("Processing file"));
/// ```
pub fn format_log(text: impl Into<String>) -> String {
    base_output(BaseColor::Log, text)
}

/// Formats text with a registered format by name.
///
/// Returns a formatted string with custom colors and styles applied.
/// Use with `cprintln!()` to output formatted text.
///
/// # Arguments
///
/// * `format_name` - Name of the registered format ("log", "ok", "err", or custom)
/// * `text` - The text to format
///
/// # Example
///
/// ```ignore
/// cprintln!("{}", format_with("warning", "This action cannot be undone"));
/// cprintln!("{}", format_with("custom_format", "Message"));
/// ```
pub fn format_with(format_name: &str, text: impl Into<String>) -> String {
    format_output(format_name, text)
}

/// Formats a string with placeholders using the "ok" (success) format.
///
///
/// This is a convenience function for creating formatted output with color-coded text and parameters.
/// The input string should contain `{}` placeholders for where variables will be inserted.
/// Main text parts are formatted in bold green, while variable values are shown in dim gray.
///
/// # Arguments
///
/// * `s` - A format string with `{}` placeholders
/// * `vars` - An array of variable values to insert at each `{}` position
///
/// # Returns
///
/// A formatted string with ANSI codes applied.
///
/// # Example
///
/// ```ignore
/// let output = color_fmt_ok("File {} saved in {}", &["config.txt", "/home/user"]);
/// console_write_log(output);
/// ```
pub fn color_fmt_ok(s: &str, vars: &[&str]) -> String {
    color_fmt_impl(TextFormat::new(BaseColor::Ok).bold(true), s, vars)
}

/// Formats a string with placeholders using the "err" (error) format.
///
/// This is a convenience function for creating error messages with color-coded text and parameters.
/// The input string should contain `{}` placeholders for where variables will be inserted.
/// Main text parts are formatted in bold red, while variable values are shown in dim gray.
///
/// # Arguments
///
/// * `s` - A format string with `{}` placeholders
/// * `vars` - An array of variable values to insert at each `{}` position
///
/// # Returns
///
/// A formatted string with ANSI codes applied.
///
/// # Example
///
/// ```ignore
/// let output = color_fmt_err("Failed to load {}", &["config.txt"]);
/// console_write_log(output);
/// ```
pub fn color_fmt_err(s: &str, vars: &[&str]) -> String {
    color_fmt_impl(TextFormat::new(BaseColor::Err).bold(true), s, vars)
}

/// Formats a string with placeholders using the "log" (info) format.
///
/// This is a convenience function for creating informational output with color-coded text and parameters.
/// The input string should contain `{}` placeholders for where variables will be inserted.
/// Main text parts are formatted in bold white/gray, while variable values are shown in dim gray.
///
/// # Arguments
///
/// * `s` - A format string with `{}` placeholders
/// * `vars` - An array of variable values to insert at each `{}` position
///
/// # Returns
///
/// A formatted string with ANSI codes applied.
///
/// # Example
///
/// ```ignore
/// let output = color_fmt_log("Processing file {}", &["data.json"]);
/// console_write_log(output);
/// ```
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
