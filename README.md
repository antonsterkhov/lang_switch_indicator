# Lang Switch Indicator

Небольшая Windows-утилита на Rust, которая показывает крупный индикатор текущей раскладки (`EN`/`RU`) в центре экрана и работает из системного трея.

## Как это работает

1. Приложение создаёт скрытое `WS_POPUP`-окно и иконку в трее.
2. Таймер (по умолчанию каждые `120` мс) проверяет раскладку активного окна через `GetKeyboardLayout`.
3. Индикатор показывается:
   - при первом запуске;
   - при смене раскладки;
   - при начале новой сессии набора текста (если пауза была >= 5 секунд).
4. Индикатор автоматически скрывается через `1200` мс (по умолчанию).
5. Через меню в трее (ПКМ по иконке) можно:
   - поставить на паузу/возобновить;
   - выбрать размер текста (`Small`/`Medium`/`Large`);
   - настроить задержки (частоту опроса, время показа, паузу сессии набора);
   - выйти из программы.

## Настройка задержек через трей

ПКМ по иконке в трее открывает меню с настройками:

- `Интервал опроса`: `80` / `120` / `200` мс  
  Как часто приложение проверяет текущую раскладку активного окна.
- `Время показа`: `600` / `1200` / `2000` мс  
  Сколько индикатор остаётся на экране после показа.
- `Пауза печати`: `2` / `5` / `8` сек  
  Пауза в наборе, после которой начало новой печати снова показывает индикатор.

Значения применяются сразу, без перезапуска.

## Что использует проект

- Язык: Rust (`edition = 2024`)
- Crate: `windows = 0.62.2`
- WinAPI (через `windows` crate):
  - окно/сообщения: `Win32_UI_WindowsAndMessaging`
  - трей: `Win32_UI_Shell`
  - клавиатура/раскладка: `Win32_UI_Input_KeyboardAndMouse`
  - отрисовка: `Win32_Graphics_Gdi`
  - загрузка модулей: `Win32_System_LibraryLoader`

## Требования

- Windows 10/11
- Установленный Rust toolchain (MSVC), `cargo` в `PATH`

Проверка:

```powershell
rustc --version
cargo --version
```

## Сборка и запуск

Запуск из исходников:

```powershell
cargo run --release
```

Сборка `.exe`:

```powershell
cargo build --release
```

Исполняемый файл:

`target\release\lang_switch_indicator.exe`

## Иконка трея (опционально)

Приложение пытается загрузить `tray.ico` из той же папки, где лежит `.exe`.  
Если файла нет, используется стандартная системная иконка.

Чтобы использовать `assets\tray.ico`:

```powershell
Copy-Item .\assets\tray.ico .\target\release\tray.ico -Force
```

## Автозапуск

### Вариант 1: папка Startup (через ярлык)

```powershell
$exe = "C:\path\to\lang_switch_indicator.exe"
$startup = [Environment]::GetFolderPath("Startup")
$wsh = New-Object -ComObject WScript.Shell
$lnk = $wsh.CreateShortcut((Join-Path $startup "Lang Switch Indicator.lnk"))
$lnk.TargetPath = $exe
$lnk.WorkingDirectory = Split-Path $exe
$lnk.Save()
```

Удаление из автозапуска:

```powershell
Remove-Item (Join-Path ([Environment]::GetFolderPath("Startup")) "Lang Switch Indicator.lnk") -ErrorAction SilentlyContinue
```

### Вариант 2: ключ реестра `HKCU\...\Run`

```powershell
$exe = "C:\path\to\lang_switch_indicator.exe"
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "LangSwitchIndicator" -Value "`"$exe`""
```

Удаление из автозапуска:

```powershell
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "LangSwitchIndicator" -ErrorAction SilentlyContinue
```

## Остановка программы

ПКМ по иконке в трее -> `Выход`.
