pub mod buffer;
pub mod engine;
pub mod frontend;
pub mod jisyo;
pub mod key;
pub mod romaji;
pub mod state;
pub mod tables;

use std::io::{Result, Write, stdout};
use std::{env, panic};

fn main() -> Result<()> {
    install_panic_hook();
    handle_args();

    const ENV_ERR: &str = "missing environment variable: ";
    let (ct, cf, j) = ("CPY_TO", "CPY_FROM", "JISYO_PATH");
    let (ct, cf, j) = (
        env::var(ct).unwrap_or_else(|_| panic!("{}{}", ENV_ERR, ct)),
        env::var(cf).unwrap_or_else(|_| panic!("{}{}", ENV_ERR, cf)),
        env::var(j).unwrap_or_else(|_| panic!("{}{}", ENV_ERR, j)),
    );
    let jisyo = crate::jisyo::Jisyo::load(&j)?;

    frontend::run(jisyo, &ct, &cf)
}

fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let mut out = stdout();
        let _ = write!(
            out,
            "{}{}",
            termion::screen::ToMainScreen,
            termion::clear::All
        );
        let _ = out.flush();
        eprintln!("panic: {}", info);
    }));
}

fn handle_args() {
    use std::process::exit;

    let bin = std::env::args()
        .next()
        .unwrap_or_else(|| "unskk".to_string());
    let arg1 = std::env::args().nth(1);

    const USAGE_HEAD: &str = "Usage:\n\
         \texport CPY_TO=\"command of output from buffer\"\n\
         \texport CPY_FROM=\"command of paste to buffer\"\n\
         \texport JISYO_PATH=\"/path/to/your/jisyo1:/path/to/your/jisyo2\"\n\
         \texec ";

    const USAGE_TAIL: &str = "\n\nOptions:\n\
         \t-h, --help     Show help\n\
         \t-v, --version  Show version\n\
         \nNotes:\n\
         \tCPY_TO/CPY_FROM are parsed as: <cmd> <args...> (no shell quoting).\n\
         \tIf you need shell features, wrap with: sh -c '...'\n";

    if let Some(arg) = arg1 {
        match arg.as_str() {
            "--version" | "-v" | "-V" => {
                let (n, v, t) = (
                    env!("CARGO_PKG_NAME"),
                    env!("CARGO_PKG_VERSION"),
                    env!("BUILD_TARGET"),
                );
                println!("{} | version: {} | target: {}", n, v, t);
                exit(0);
            }
            "--help" | "-h" | "-H" => {
                println!("{}{}{}", USAGE_HEAD, bin, USAGE_TAIL);
                exit(0);
            }
            _ => {
                eprintln!("unknown option: {}", arg);
                eprintln!("try --help");
                exit(1);
            }
        }
    }
}
