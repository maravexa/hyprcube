# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Release infrastructure

- The Release workflow can be manually dispatched for an existing signed tag
  when repository Actions were disabled during the original tag push.

## [0.1.0] - 2026-09-02

### Added

- Larger application typography and slider controls with synchronized,
  editable numeric value fields.
- Complete About content covering HyprCube, companion projects, runtime
  information, licenses, and principal dependency attributions.
- Current display discovery plus resolution, refresh-rate, scaling,
  transform, enablement, and visual layout controls.
- A dynamic Hyprpaper panel with multi-monitor layout and image previews,
  per-monitor wallpaper paths and fit modes, live apply, and managed config.
- Expanded HyprSaver configuration, searchable installed resources and
  playlists, a native preview launcher, and snapshot-friendly theme overrides
  that can coordinate palette/playlists with Hyprpaper wallpaper selection.
- Git-backed configuration history with revision navigation, section-filtered
  diff previews, restore support, and named snapshots.
- Versioned HyprDeck configuration-schema integration with validation and
  atomic saves.
- Release packaging for crates.io, AUR, Debian, RPM, and portable archives.

### Changed

- HyprDeck settings now expose theme selection and show only the module
  controls relevant to the selected theme when supported by its schema.

### Fixed

- Updated transitive Wayland XML and concurrency dependencies to releases
  covering the RustSec advisories known at release time.
- Custom Display and Appearance controls route pointer drag and keyboard input
  correctly, including exact slider value entry.
- Display discovery falls back to `hyprctl -j monitors` when direct socket
  discovery is unavailable instead of presenting a blank panel.
