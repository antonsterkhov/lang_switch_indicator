fn version4(input: &str) -> String {
    let mut parts: Vec<&str> = input.split('.').collect();
    while parts.len() < 4 {
        parts.push("0");
    }
    parts.truncate(4);
    parts.join(".")
}

fn main() {
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let file_version = version4(&pkg_version);

    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/tray.ico");
    res.set("FileDescription", "Индикатор раскладки клавиатуры");
    res.set("ProductName", "Lang Switch Indicator");
    res.set("CompanyName", "Anton Sterkhov");
    res.set("LegalCopyright", "© 2026 Anton Sterkhov");
    res.set("OriginalFilename", "lang_switch_indicator.exe");
    res.set("FileVersion", &file_version);
    res.set("ProductVersion", &file_version);
    res.compile().expect("failed to compile Windows resources");
}
