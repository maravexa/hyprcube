mod cli;
mod config;
mod preview;
mod registry;

use config::AppConfig;
use hyprcube_core::palette::Palette;
use preview::PreviewEngine;
use registry::PanelRegistry;
use tracing_subscriber::EnvFilter;

fn main() {
    // 1. Init tracing with RUST_LOG / env filter, default to hyprcube=debug.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("hyprcube=debug")),
        )
        .init();

    // Parse CLI args (--help and --version exit early).
    let args = cli::parse();

    // Handle --reset-config.
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

    // Handle --daemon placeholder.
    if args.daemon {
        println!("daemon mode not yet implemented");
        return;
    }

    // 2. Load or create AppConfig.
    let mut app_config = AppConfig::load();
    tracing::debug!("loaded config: {app_config:?}");

    // 3. Load or create Palette (save default if missing).
    let _palette = match Palette::load(None) {
        Ok(p) => {
            tracing::debug!("loaded palette");
            p
        }
        Err(_) => {
            tracing::info!("no palette found, saving defaults");
            let p = Palette::default();
            if let Err(e) = p.save(None) {
                tracing::warn!("failed to save default palette: {e}");
            }
            p
        }
    };

    // 4. Build PanelRegistry.
    let mut registry = PanelRegistry::new();

    // Apply --panel flag if given.
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

    // 5. Build PreviewEngine.
    let _preview = PreviewEngine::new();

    // 6. Log panel count and titles.
    let panel_info = registry.available_panels();
    let titles: Vec<&str> = panel_info.iter().map(|(_, t, _)| *t).collect();
    tracing::info!("{} panels available: {}", panel_info.len(), titles.join(", "));

    // 7. TODO: Wayland event loop will go here.
    tracing::debug!("Wayland event loop will go here");

    // 8. Save AppConfig on exit.
    if let Err(e) = app_config.save() {
        tracing::error!("failed to save config on exit: {e}");
    }
}
