[![GitHub](https://img.shields.io/badge/GitHub-consoletools-blue?logo=github)](https://github.com/yourname/consoletools)
[![Crates.io](https://img.shields.io/crates/v/consoletools.svg)](https://crates.io/crates/consoletools)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

# consoletools

[ **Russian**](README_RU.md)

An interactive terminal console with a persistent input line `>>`, scrolling message output above that line, and command registration support.

## Formatting Model

The library has only 3 fixed base colors for built-in output levels:

- `BaseColor::Log`
- `BaseColor::Ok`
- `BaseColor::Err`

The base colors themselves are immutable and cannot be extended or overridden. For custom formats, there is a separate `CustomColor` palette from which you can build any number of output schemes.

Available `CustomColor`:

- `Black`, `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`
- `BrightBlack`, `BrightRed`, `BrightGreen`, `BrightYellow`, `BrightBlue`, `BrightMagenta`, `BrightCyan`, `BrightWhite`

Each format can include:

- `bold`
- `italic`
- `dim`
- `underline`
- `reverse`
- `strike`
- a color from `CustomColor`

Formats are registered by name and then used in output macros and functions. The base `log/ok/err` remain fixed and cannot be changed via `register_output_format`.

## Installation

```toml
[dependencies]
consoletools = { path = "." }
```

## Quick Start

```rust
use consoletools::{
    cprintln_err, cprintln_fmt, cprintln_log, cprintln_ok, install_global_console_handle,
    register_output_format, CommandConsole, CommandOutput, CustomColor, TextFormat,
};
use std::thread;
use std::time::Duration;

fn main() {
    let mut console = CommandConsole::new(">>");
    install_global_console_handle(console.handle());

    let _ = register_output_format("prompt", TextFormat::custom(CustomColor::BrightCyan).bold(true).underline(true));
    let _ = register_output_format("event", TextFormat::custom(CustomColor::BrightMagenta).italic(true).underline(true));
    let _ = register_output_format("warning", TextFormat::custom(CustomColor::BrightYellow).bold(true).reverse(true));
    let _ = register_output_format("accent", TextFormat::custom(CustomColor::BrightBlue).bold(true).strike(true));
    let _ = register_output_format("note", TextFormat::custom(CustomColor::BrightGreen).dim(true).italic(true));

    thread::spawn(move || {
        let mut i = 1_u64;
        loop {
            cprintln_log!("tick {}", i);
            if i % 5 == 0 {
                cprintln_ok!("ok {}", i);
            }
            if i % 9 == 0 {
                cprintln_err!("err {}", i);
            }
            if i % 7 == 0 {
                cprintln_fmt!("event", "custom event {}", i);
            }
            if i % 13 == 0 {
                cprintln_fmt!("accent", "accent {}", i);
            }
            i += 1;
            thread::sleep(Duration::from_secs(1));
        }
    });

    console.add_command("echo", "Print the provided text", |args| {
        if args.is_empty() {
            CommandOutput::Error("Usage: echo <text>".to_string())
        } else {
            CommandOutput::Info(args.join(" "))
        }
    });

    console.add_command("sum", "Sum numbers", |args| {
        let mut total = 0.0_f64;
        for arg in args {
            match arg.parse::<f64>() {
                Ok(v) => total += v,
                Err(_) => return CommandOutput::Error(format!("Not a number: {}", arg)),
            }
        }
        CommandOutput::Success(format!("Sum: {}", total))
    });

    if let Err(err) = console.run() {
        eprintln!("Console error: {}", err);
    }
}
```

## Public API

- `CommandConsole::new(prompt)` - create a console
- `CommandConsole::add_command(name, description, handler)` - register a command
- `CommandConsole::handle()` - get a thread-safe handle for background threads
- `CommandConsole::run()` - start the input and output loop
- `CommandConsole::save_to_file(path)` - save the current console log to a file
- `CommandConsole::enable_autosave(path)` - enable dynamic saving of new lines
- `CommandConsole::disable_autosave()` - disable dynamic saving
- `install_global_console_handle(handle)` - enable output macros
- `register_output_format(name, format) -> bool` - create or replace a custom format (returns `false` for `log/ok/err`)
- `TextFormat::new(BaseColor::...)` - create a format based on one of the 3 fixed colors
- `TextFormat::custom(CustomColor::...)` - create a format based on the extended palette
- `TextFormat::bold/italic/dim/underline/reverse/strike(...)` - configure styles
- `console_write_log/ok/err(text)` - direct output by level
- `console_write_format(name, text)` - output through any registered format
- `cprintln_log!`, `cprintln_ok!`, `cprintln_err!` - output macros by level
- `cprintln_fmt!(name, ...)` - output macro through any format
- `cprint!` - thread-safe output without line break

## Built-in Commands

- `help` - show help
- `clear` - clear the log
- `save <path/filename>` - save the current log to a file
- `autosave on <path/filename>` - enable dynamic saving
- `autosave off` - disable dynamic saving
- `exit` or `quit` - exit

## Notes

- Before using `cprint!` and `cprintln_*` macros, you need to call `install_global_console_handle` once.
- Base colors `log/ok/err` are fixed and can only be selected via `BaseColor::Log`, `BaseColor::Ok`, and `BaseColor::Err`.
- For custom formats, use `CustomColor`, `register_output_format`, and `cprintln_fmt!`.
- The `prompt` format can be overridden via `register_output_format("prompt", ...)`.
- The `save` command saves the log to a text file and automatically creates missing directories.
- The `autosave on` command first saves the entire current log, then automatically appends each new line.

