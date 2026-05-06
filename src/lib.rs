pub mod core;

pub use core::{
    color_fmt_err, color_fmt_log, color_fmt_ok, console_write_log, console_write_raw, create_format,
    format_err, format_log, format_ok, format_output, format_with, install_global_console_handle,
    register_output_format, BaseColor, CommandConsole, CommandOutput, ConsoleHandle, CustomColor,
    TextFormat,
};
