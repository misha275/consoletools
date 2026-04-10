pub mod core;

pub use core::{
    color_fmt_err, color_fmt_log, color_fmt_ok, console_write_err, console_write_format,
    console_write_log, console_write_ok, console_write_raw, format_output,
    install_global_console_handle, register_output_format, BaseColor, CommandConsole,
    CommandOutput, ConsoleHandle, CustomColor, TextFormat,
};
