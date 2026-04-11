/// Parsed command-line arguments.
pub struct CliArgs {
    /// Jump directly to a panel by title.
    pub panel: Option<String>,
    /// Daemon mode flag (placeholder).
    pub daemon: bool,
    /// Delete config and exit.
    pub reset_config: bool,
}

/// Parse CLI arguments. Handles --help and --version by printing and exiting.
pub fn parse() -> CliArgs {
    let mut args = std::env::args().skip(1);
    let mut cli = CliArgs {
        panel: None,
        daemon: false,
        reset_config: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--version" | "-V" => {
                println!("hyprcube {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--panel" => {
                cli.panel = args.next().or_else(|| {
                    eprintln!("error: --panel requires a panel name");
                    std::process::exit(1);
                });
            }
            "--daemon" => {
                cli.daemon = true;
            }
            "--reset-config" => {
                cli.reset_config = true;
            }
            other => {
                eprintln!("error: unknown argument: {other}");
                eprintln!("try 'hyprcube --help' for usage");
                std::process::exit(1);
            }
        }
    }

    cli
}

fn print_help() {
    println!(
        "\
hyprcube {} — Hyprland settings manager

USAGE:
    hyprcube [OPTIONS]

OPTIONS:
    --help, -h          Print this help message
    --version, -V       Print version
    --panel <name>      Jump directly to a panel by title
    --daemon            Run in daemon mode (not yet implemented)
    --reset-config      Delete config file and exit",
        env!("CARGO_PKG_VERSION")
    );
}
