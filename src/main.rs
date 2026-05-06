use consoletools::{
	install_global_console_handle, register_output_format, CommandConsole, CommandOutput,
	CustomColor, TextFormat, cprintln, format_ok, format_err, format_with,
};
use std::thread;
use std::time::Duration;

fn main() {
	let mut console = CommandConsole::new(">>");
	install_global_console_handle(console.handle());

	register_output_format("prompt", TextFormat::custom(CustomColor::BrightCyan).bold(true).underline(true));
	register_output_format(
		"event",
		TextFormat::custom(CustomColor::BrightMagenta).italic(true).underline(true),
	);
	register_output_format(
		"warning",
		TextFormat::custom(CustomColor::BrightYellow).bold(true).reverse(true),
	);
	register_output_format(
		"accent",
		TextFormat::custom(CustomColor::BrightBlue).bold(true).strike(true),
	);
	register_output_format(
		"note",
		TextFormat::custom(CustomColor::BrightGreen).dim(true).italic(true),
	);

	thread::spawn(move || {
		let mut tick: u64 = 1;
		loop {
			cprintln!("Background thread: event #{}", tick);
			if tick % 5 == 0 {
				cprintln!("{}", format_ok("Background thread active"));
			}
			if tick % 9 == 0 {
				cprintln!("{}", format_err("Test error from background thread"));
			}
			if tick % 7 == 0 {
				cprintln!("{}", format_with("event", format!("Event with custom format #{}", tick)));
			}
			if tick % 11 == 0 {
				cprintln!("{}", format_with("warning", format!("Custom warning #{}", tick)));
			}
			if tick % 13 == 0 {
				cprintln!("{}", format_with("accent", format!("Accent format #{}", tick)));
			}
			if tick % 17 == 0 {
				cprintln!("{}", format_with("note", format!("Additional format #{}", tick)));
			}
			tick += 1;
			thread::sleep(Duration::from_secs(2));
		}
	});

	console.add_command("echo", "Print the passed text", |args| {
		if args.is_empty() {
			CommandOutput::Error("Usage: echo <text>".to_string())
		} else {
			CommandOutput::Info(args.join(" "))
		}
	});

	console.add_command("sum", "Sum numbers: sum 1 2 3", |args| {
		if args.is_empty() {
			return CommandOutput::Error("Usage: sum <n1> <n2> ...".to_string());
		}

		let mut total = 0.0_f64;
		for arg in args {
			match arg.parse::<f64>() {
				Ok(value) => total += value,
				Err(_) => {
					return CommandOutput::Error(format!("Not a number: {}", arg));
				}
			}
		}

		CommandOutput::Success(format!("Sum: {}", total))
	});

	console.add_command("about", "Information about the application", |_| {
		CommandOutput::Info("consoletools: interactive command console".to_string())
	});

	if let Err(err) = console.run() {
		eprintln!("Console error: {}", err);
	}
}
