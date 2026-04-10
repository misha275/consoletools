# consoletools

Интерактивная консоль для терминала с постоянной строкой ввода `>>`, выводом сообщений выше этой строки и регистрацией команд.

## Модель форматирования

В библиотеке есть только 3 фиксированных базовых цвета для встроенных уровней вывода:

- `BaseColor::Log`
- `BaseColor::Ok`
- `BaseColor::Err`

Сами базовые цвета не расширяются и не переопределяются. Для пользовательских форматов есть отдельная палитра `CustomColor`, из которой можно собрать любое количество схем вывода.

Доступные `CustomColor`:

- `Black`, `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`
- `BrightBlack`, `BrightRed`, `BrightGreen`, `BrightYellow`, `BrightBlue`, `BrightMagenta`, `BrightCyan`, `BrightWhite`

Каждый формат может включать:

- `bold`
- `italic`
- `dim`
- `underline`
- `reverse`
- `strike`
- цвет из `CustomColor`

Форматы регистрируются по имени и затем используются в макросах и функциях вывода. Базовые `log/ok/err` остаются фиксированными и через `register_output_format` не меняются.

## Установка

```toml
[dependencies]
consoletools = { path = "." }
```

## Быстрый старт

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

    console.add_command("echo", "Печатает переданный текст", |args| {
        if args.is_empty() {
            CommandOutput::Error("Использование: echo <текст>".to_string())
        } else {
            CommandOutput::Info(args.join(" "))
        }
    });

    console.add_command("sum", "Суммирует числа", |args| {
        let mut total = 0.0_f64;
        for arg in args {
            match arg.parse::<f64>() {
                Ok(v) => total += v,
                Err(_) => return CommandOutput::Error(format!("Не число: {}", arg)),
            }
        }
        CommandOutput::Success(format!("Сумма: {}", total))
    });

    if let Err(err) = console.run() {
        eprintln!("Ошибка консоли: {}", err);
    }
}
```

## Публичный API

- `CommandConsole::new(prompt)` - создать консоль
- `CommandConsole::add_command(name, description, handler)` - зарегистрировать команду
- `CommandConsole::handle()` - получить потокобезопасный handle для фоновых потоков
- `CommandConsole::run()` - запустить цикл ввода и вывода
- `CommandConsole::save_to_file(path)` - сохранить текущий лог консоли в файл
- `CommandConsole::enable_autosave(path)` - включить динамическое сохранение новых строк
- `CommandConsole::disable_autosave()` - выключить динамическое сохранение
- `install_global_console_handle(handle)` - включить макросы вывода
- `register_output_format(name, format) -> bool` - создать или заменить пользовательский формат (вернет `false` для `log/ok/err`)
- `TextFormat::new(BaseColor::...)` - создать формат на базе одного из 3 фиксированных цветов
- `TextFormat::custom(CustomColor::...)` - создать формат на базе расширенной палитры
- `TextFormat::bold/italic/dim/underline/reverse/strike(...)` - настроить стили
- `console_write_log/ok/err(text)` - прямой вывод по уровням
- `console_write_format(name, text)` - вывод через любой зарегистрированный формат
- `cprintln_log!`, `cprintln_ok!`, `cprintln_err!` - макросы вывода по уровням
- `cprintln_fmt!(name, ...)` - макрос вывода через любой формат
- `cprint!` - потокобезопасный вывод без перевода строки

## Встроенные команды

- `help` - показать список команд
- `clear` - очистить лог
- `save <путь/имя_файла>` - сохранить текущий лог в файл
- `autosave on <путь/имя_файла>` - включить динамическое сохранение
- `autosave off` - выключить динамическое сохранение
- `exit` или `quit` - выход

## Примечания

- Перед использованием `cprint!` и макросов `cprintln_*` нужно один раз вызвать `install_global_console_handle`.
- Базовые цвета `log/ok/err` фиксированы и выбираются только через `BaseColor::Log`, `BaseColor::Ok` и `BaseColor::Err`.
- Для пользовательских форматов используйте `CustomColor`, `register_output_format` и `cprintln_fmt!`.
- Формат `prompt` можно переопределить через `register_output_format("prompt", ...)`.
- Команда `save` сохраняет лог в текстовый файл и автоматически создает отсутствующие папки.
- Команда `autosave on` сначала сохраняет текущий лог целиком, затем автоматически дописывает каждую новую строку.
