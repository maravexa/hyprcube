mod cli;
mod config;
mod history;
mod preview;
mod registry;
mod shared_config;
mod sidebar;
mod wayland;

use config::AppConfig;
use preview::PreviewEngine;
use registry::PanelRegistry;
use tracing_subscriber::EnvFilter;

fn main() {
    // 1. Init tracing with RUST_LOG / env filter, default to hyprcube=debug.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("hyprcube=debug")),
        )
        .init();

    let args = cli::parse();

    if args.reset_config {
        match AppConfig::delete() {
            Ok(()) => {
                tracing::info!("config file deleted");
                println!("Config reset successfully.");
            }
            Err(e) => {
                tracing::error!("failed to delete config: {e}");
                eprintln!("error: failed to delete config: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.daemon {
        println!("daemon mode not yet implemented");
        return;
    }

    // 2. Load or create AppConfig.
    let mut app_config = AppConfig::load();
    tracing::debug!("loaded config: {app_config:?}");

    // 3. Build PanelRegistry (loads hyprland.conf + palette internally).
    let mut registry = PanelRegistry::new();

    if let Some(ref name) = args.panel {
        if let Some(idx) = registry.find_by_title(name) {
            registry.set_active(idx);
            app_config.last_panel = idx;
        } else {
            tracing::warn!("panel not found: {name}");
            eprintln!("warning: panel '{name}' not found, using default");
        }
    } else if app_config.last_panel < registry.len() {
        registry.set_active(app_config.last_panel);
    }

    // 4. Build PreviewEngine.
    let preview = PreviewEngine::new();

    let panel_info = registry.available_panels();
    let titles: Vec<&str> = panel_info.iter().map(|(_, t, _)| *t).collect();
    tracing::info!(
        "{} panels available: {}",
        panel_info.len(),
        titles.join(", ")
    );

    // 5. Run the Wayland event loop.
    let app_config = match wayland::run(registry, preview, app_config) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    // 6. Save AppConfig on exit.
    if let Err(e) = app_config.save() {
        tracing::error!("failed to save config on exit: {e}");
    }
}
