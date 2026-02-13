pub mod buffer;
pub mod engine;
pub mod frontend;
pub mod jisyo;
pub mod key;
pub mod romaji;
pub mod state;
pub mod tables;
pub mod util;

use std::io::Result;
use std::{
    env,
    fs::{File, OpenOptions},
    panic,
};
use termion::{
    raw::{IntoRawMode, RawTerminal},
    screen::{AlternateScreen, IntoAlternateScreen},
};

const DEVICE: &str = "/dev/tty";

fn main() -> Result<()> {
    install_panic_hook();
    handle_args();
    let ui = open_alt_raw_term()?;
    let input = open_input()?;
    let (sh, ct, cf, j) = handle_env();
    let jisyo = crate::jisyo::Jisyo::load(&j)?;
    frontend::run(ui, input, jisyo, &sh, &ct, &cf)
}

fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        if let Ok(mut tty) = OpenOptions::new().write(true).open(DEVICE) {
            let _ = frontend::cleanup(&mut tty);
        }
        eprintln!("{}", info);
    }));
}

fn open_alt_raw_term() -> Result<AlternateScreen<RawTerminal<File>>> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(DEVICE)?
        .into_raw_mode()?
        .into_alternate_screen()
}

fn open_input() -> Result<File> {
    OpenOptions::new().read(true).open(DEVICE)
}

fn handle_args() {
    use std::process::exit;
    let mut args = std::env::args();
    let arg1 = args.nth(1);

    if let Some(arg) = arg1 {
        match arg.as_str() {
            "--version" | "-v" | "-V" => {
                println!(
                    "{} | version: {} | target: {}",
                    env!("CARGO_PKG_NAME"),
                    env!("CARGO_PKG_VERSION"),
                    env!("BUILD_TARGET")
                );
                exit(0);
            }
            _ => {
                eprintln!("unknown option: {}", arg);
                exit(1);
            }
        }
    }
}

fn handle_env() -> (String, String, String, String) {
    const ENV_ERR: &str = "missing environment variable: ";
    let (sh, ct, cf, j) = ("SHELL", "CPY_TO", "CPY_FROM", "JISYO_PATH");
    (
        env::var(sh).unwrap_or_else(|_| panic!("{}{}", ENV_ERR, sh)),
        env::var(ct).unwrap_or_else(|_| panic!("{}{}", ENV_ERR, ct)),
        env::var(cf).unwrap_or_else(|_| panic!("{}{}", ENV_ERR, cf)),
        env::var(j).unwrap_or_else(|_| panic!("{}{}", ENV_ERR, j)),
    )
}
