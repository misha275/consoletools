use consoletools::{
	install_global_console_handle, register_output_format, CommandConsole, CommandOutput,
	CustomColor, TextFormat,
};
use consoletools::{cprintln_err, cprintln_fmt, cprintln_log, cprintln_ok};
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
			cprintln_log!("Фоновый поток: событие #{}", tick);
			if tick % 5 == 0 {
				cprintln_ok!("Фоновый поток активен");
			}
			if tick % 9 == 0 {
				cprintln_err!("Тестовая ошибка от фонового потока");
			}
			if tick % 7 == 0 {
				cprintln_fmt!("event", "Событие с пользовательским форматом #{}", tick);
			}
			if tick % 11 == 0 {
				cprintln_fmt!("warning", "Пользовательский warning #{}", tick);
			}
			if tick % 13 == 0 {
				cprintln_fmt!("accent", "Акцентный формат #{}", tick);
			}
			if tick % 17 == 0 {
				cprintln_fmt!("note", "Дополнительный формат #{}", tick);
			}
			tick += 1;
			thread::sleep(Duration::from_secs(2));
		}
	});

	console.add_command("echo", "Печатает переданный текст", |args| {
		if args.is_empty() {
			CommandOutput::Error("Использование: echo <текст>".to_string())
		} else {
			CommandOutput::Info(args.join(" "))
		}
	});

	console.add_command("sum", "Суммирует числа: sum 1 2 3", |args| {
		if args.is_empty() {
			return CommandOutput::Error("Использование: sum <n1> <n2> ...".to_string());
		}

		let mut total = 0.0_f64;
		for arg in args {
			match arg.parse::<f64>() {
				Ok(value) => total += value,
				Err(_) => {
					return CommandOutput::Error(format!("Не число: {}", arg));
				}
			}
		}

		CommandOutput::Success(format!("Сумма: {}", total))
	});

	console.add_command("about", "Информация о приложении", |_| {
		CommandOutput::Info("consoletools: интерактивная командная консоль".to_string())
	});

	if let Err(err) = console.run() {
		eprintln!("Ошибка консоли: {}", err);
	}
}
