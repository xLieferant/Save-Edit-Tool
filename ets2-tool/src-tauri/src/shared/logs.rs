use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;

// Wichtig: Dieses Makro exportiert das globale log!-Makro
#[macro_export]
macro_rules! dev_log {
    ($($arg:tt)*) => {{
        $crate::shared::logs::write_log(format!($($arg)*));
    }};
}

pub fn write_log(msg: String) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] {}\n", timestamp, msg);

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("ets2_tool.log")
        .expect("Kann ets2_tool.log nicht öffnen");

    let _ = file.write_all(entry.as_bytes());

    println!("{}", entry);
}
