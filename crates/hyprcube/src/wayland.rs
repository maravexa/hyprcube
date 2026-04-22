use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        WaylandSurface,
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm, ShmHandler,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

use tiny_skia::Pixmap;

use crate::config::AppConfig;
use crate::preview::PreviewEngine;
use crate::registry::PanelRegistry;
use crate::sidebar;
use hyprcube_core::color::Color;
use hyprcube_core::layout::Rect;
use hyprcube_core::palette::Palette;
use hyprcube_core::text::{RenderContext, TextRenderer};
use hyprcube_core::widget::Event as WidgetEvent;
use hyprcube_core::widget::WidgetAction;
use hyprcube_panels::form::FormLayout;

#[derive(Debug, thiserror::Error)]
pub enum WaylandError {
    #[error("HyprCube requires a Wayland compositor")]
    NoWayland,
    #[error("wayland initialization failed: {0}")]
    Init(String),
    #[error("wayland dispatch error: {0}")]
    Dispatch(String),
}

pub struct HyprCubeApp {
    // SCTK state
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    _compositor_state: CompositorState,
    _xdg_shell: XdgShell,
    shm: Shm,

    // Window
    window: Window,
    pool: SlotPool,
    buffer: Option<Buffer>,

    // App state
    registry: PanelRegistry,
    preview: PreviewEngine,
    config: AppConfig,
    palette: Palette,

    // Text rendering
    text_renderer: TextRenderer,

    // Window dimensions
    width: u32,
    height: u32,
    needs_redraw: bool,
    first_configure: bool,
    exit: bool,

    // Form layout (rebuilt when the active panel or window width changes)
    form: Option<FormLayout>,
    form_panel_idx: usize,
    form_content_width: f32,

    // Pointer state
    pointer: Option<wl_pointer::WlPointer>,
    pointer_x: f64,
    pointer_y: f64,
    hover_panel: Option<usize>,

    // Keyboard state
    keyboard: Option<wl_keyboard::WlKeyboard>,
}

/// Map a raw XKB keysym value to a key name string matching what `Widget::event` expects.
fn keysym_name(raw: u32) -> String {
    match raw {
        0xff08 => "BackSpace".into(),
        0xffff => "Delete".into(),
        0xff51 => "Left".into(),
        0xff52 => "Up".into(),
        0xff53 => "Right".into(),
        0xff54 => "Down".into(),
        0xff50 => "Home".into(),
        0xff57 => "End".into(),
        0xff0d | 0xff8d => "Return".into(),
        0xff1b => "Escape".into(),
        0xff09 => "Tab".into(),
        other => format!("{other:#010x}"),
    }
}

impl HyprCubeApp {
    fn content_bounds(width: u32, height: u32) -> Rect {
        const PADDING: f32 = 16.0;
        let content_x = sidebar::SIDEBAR_WIDTH + PADDING;
        Rect {
            x: content_x,
            y: PADDING,
            width: (width as f32 - content_x - PADDING).max(0.0),
            height: (height as f32 - PADDING * 2.0).max(0.0),
        }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>) {
        let width = self.width;
        let height = self.height;
        if width == 0 || height == 0 {
            return;
        }

        // Rebuild form when panel or content width changes.
        const PADDING: f32 = 16.0;
        let content_x = sidebar::SIDEBAR_WIDTH;
        let content_w = (width as f32 - content_x - PADDING * 2.0).max(0.0);
        let content_h = height as f32 - PADDING * 2.0;
        let active_idx = self.registry.active_index();

        if self.form.is_none()
            || active_idx != self.form_panel_idx
            || (content_w - self.form_content_width).abs() > 0.5
        {
            let mut form = self.registry.active_panel().build_form(content_w);
            form.set_viewport_height(content_h);
            self.form = Some(form);
            self.form_panel_idx = active_idx;
            self.form_content_width = content_w;
        } else if let Some(f) = &mut self.form {
            f.set_viewport_height(content_h);
        }

        // Render to a temporary pixmap first (avoids borrow conflicts with pool)
        let mut pixmap = match Pixmap::new(width, height) {
            Some(p) => p,
            None => return,
        };

        Self::render_to(
            &mut pixmap,
            &self.registry,
            &self.palette,
            &mut self.text_renderer,
            self.hover_panel,
            width,
            height,
            self.form.as_ref(),
        );

        // Create buffer from the pool
        let stride = width as i32 * 4;
        let pool_size = stride as usize * height as usize;
        if self.pool.len() < pool_size {
            self.pool.resize(pool_size).expect("resize shm pool");
        }

        let (buffer, canvas) = self
            .pool
            .create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
            .expect("create buffer");

        // Convert RGBA (tiny_skia) -> BGRA (Wayland ARGB8888 on little-endian)
        let src = pixmap.data();
        for (dst, src) in canvas.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            dst[0] = src[2]; // B
            dst[1] = src[1]; // G
            dst[2] = src[0]; // R
            dst[3] = src[3]; // A
        }

        // Attach buffer to surface and commit
        let surface = self.window.wl_surface();
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();

        self.buffer = Some(buffer);
        self.needs_redraw = false;
    }

    fn render_to(
        pixmap: &mut Pixmap,
        registry: &PanelRegistry,
        palette: &Palette,
        text_renderer: &mut TextRenderer,
        hover_panel: Option<usize>,
        width: u32,
        height: u32,
        form: Option<&FormLayout>,
    ) {
        let bg = Color::from_hex(&palette.colors.background)
            .unwrap_or(Color::new(0.118, 0.118, 0.18, 1.0));
        let surface_c = Color::from_hex(&palette.colors.surface)
            .unwrap_or(Color::new(0.192, 0.196, 0.267, 1.0));
        let accent_c = Color::from_hex(&palette.colors.accent)
            .unwrap_or(Color::new(0.537, 0.706, 0.98, 1.0));
        let fg = Color::from_hex(&palette.colors.foreground)
            .unwrap_or(Color::new(0.804, 0.839, 0.957, 1.0));
        let overlay_c = Color::from_hex(&palette.colors.overlay)
            .unwrap_or(Color::new(0.424, 0.439, 0.525, 1.0));

        // Clear background
        pixmap.fill(bg.to_skia());

        // Determine active visual index
        let panels = registry.available_panels();
        let titles: Vec<&str> = panels.iter().map(|(_, t, _)| *t).collect();
        let active_title = registry.active_panel().title();
        let active_visual = titles.iter().position(|t| *t == active_title).unwrap_or(0);

        let font_size = (palette.fonts.size as f32).max(8.0).min(32.0);

        // Paint sidebar
        let mut pm = pixmap.as_mut();
        sidebar::paint(
            &mut pm,
            text_renderer,
            &titles,
            active_visual,
            hover_panel,
            surface_c,
            accent_c,
            fg,
            overlay_c,
            height as f32,
            font_size,
        );

        // Paint active panel content area
        const PADDING: f32 = 16.0;
        let content_x = sidebar::SIDEBAR_WIDTH + PADDING;
        let content_width = width as f32 - content_x - PADDING;
        if content_width > 0.0 {
            let bounds = Rect {
                x: content_x,
                y: PADDING,
                width: content_width,
                height: height as f32 - PADDING * 2.0,
            };
            let mut ctx = RenderContext { text: text_renderer, palette };
            if let Some(f) = form {
                f.paint(bounds, &mut ctx, &mut pm);
            } else {
                registry.active_panel().paint(bounds, &mut pm, &mut ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SCTK trait implementations
// ---------------------------------------------------------------------------

impl CompositorHandler for HyprCubeApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if self.needs_redraw {
            self.draw(qh);
        }
    }
}

impl OutputHandler for HyprCubeApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for HyprCubeApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SeatHandler for HyprCubeApp {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            if let Ok(pointer) = self.seat_state.get_pointer(qh, &seat) {
                self.pointer = Some(pointer);
            }
        }
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            if let Ok(keyboard) = self.seat_state.get_keyboard(qh, &seat, None) {
                self.keyboard = Some(keyboard);
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
        if capability == Capability::Keyboard {
            if let Some(keyboard) = self.keyboard.take() {
                keyboard.release();
            }
        }
    }

    fn remove_seat(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }
}

impl PointerHandler for HyprCubeApp {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_x = event.position.0;
                    self.pointer_y = event.position.1;

                    // Update hover state in sidebar
                    if self.pointer_x < sidebar::SIDEBAR_WIDTH as f64 {
                        let panel_count = self.registry.available_panels().len();
                        let new_hover = sidebar::hit_test(self.pointer_y as f32, panel_count);
                        if new_hover != self.hover_panel {
                            self.hover_panel = new_hover;
                            self.needs_redraw = true;
                        }
                    } else if self.hover_panel.is_some() {
                        self.hover_panel = None;
                        self.needs_redraw = true;
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    // BTN_LEFT = 0x110
                    if button == 0x110 {
                        if self.pointer_x < sidebar::SIDEBAR_WIDTH as f64 {
                            let panel_count = self.registry.available_panels().len();
                            if let Some(idx) =
                                sidebar::hit_test(self.pointer_y as f32, panel_count)
                            {
                                self.registry.set_active(idx);
                                self.config.last_panel = idx;
                                self.needs_redraw = true;
                            }
                        } else if let Some(form) = &mut self.form {
                            let bounds = Self::content_bounds(self.width, self.height);
                            let ev = WidgetEvent::PointerDown {
                                x: self.pointer_x as f32,
                                y: self.pointer_y as f32,
                            };
                            let resp = form.event(&ev, bounds);
                            if resp.consumed {
                                self.needs_redraw = true;
                            }
                            if let Some(WidgetAction::ValueChanged { ref key, ref value }) =
                                resp.action
                            {
                                let _ = self.preview.apply_preview(key, value);
                            }
                        }
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if button == 0x110 && self.pointer_x >= sidebar::SIDEBAR_WIDTH as f64 {
                        if let Some(form) = &mut self.form {
                            let bounds = Self::content_bounds(self.width, self.height);
                            let ev = WidgetEvent::PointerUp {
                                x: self.pointer_x as f32,
                                y: self.pointer_y as f32,
                            };
                            let resp = form.event(&ev, bounds);
                            if resp.consumed {
                                self.needs_redraw = true;
                            }
                            if let Some(WidgetAction::ValueChanged { ref key, ref value }) =
                                resp.action
                            {
                                let _ = self.preview.apply_preview(key, value);
                            }
                        }
                    }
                }
                PointerEventKind::Axis { vertical, .. } => {
                    if self.pointer_x >= sidebar::SIDEBAR_WIDTH as f64 {
                        if let Some(form) = &mut self.form {
                            let bounds = Self::content_bounds(self.width, self.height);
                            let ev = WidgetEvent::Scroll { dx: 0.0, dy: vertical.absolute as f32 };
                            let resp = form.event(&ev, bounds);
                            if resp.consumed {
                                self.needs_redraw = true;
                            }
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    if self.hover_panel.is_some() {
                        self.hover_panel = None;
                        self.needs_redraw = true;
                    }
                }
                _ => {}
            }
        }

        if self.needs_redraw {
            self.draw(qh);
        }
    }
}

impl KeyboardHandler for HyprCubeApp {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // XKB_KEY_Escape = 0xff1b
        if event.keysym.raw() == 0xff1b {
            self.exit = true;
            return;
        }

        if let Some(form) = &mut self.form {
            let key_name = keysym_name(event.keysym.raw());
            let utf8 = event.utf8.clone();
            let ev = WidgetEvent::KeyPress { key: key_name, utf8 };
            let resp = form.event(&ev, Self::content_bounds(self.width, self.height));
            if resp.consumed {
                self.needs_redraw = true;
                self.draw(qh);
            }
            if let Some(WidgetAction::ValueChanged { ref key, ref value }) = resp.action {
                let _ = self.preview.apply_preview(key, value);
            }
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}

impl WindowHandler for HyprCubeApp {
    fn request_close(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
    ) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let (new_w, new_h) = configure.new_size;
        if let (Some(w), Some(h)) = (new_w, new_h) {
            self.width = w.get();
            self.height = h.get();
        }

        self.first_configure = false;
        self.needs_redraw = true;
        self.draw(qh);
    }
}

impl ProvidesRegistryState for HyprCubeApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(HyprCubeApp);
delegate_output!(HyprCubeApp);
delegate_shm!(HyprCubeApp);
delegate_seat!(HyprCubeApp);
delegate_pointer!(HyprCubeApp);
delegate_keyboard!(HyprCubeApp);
delegate_xdg_shell!(HyprCubeApp);
delegate_xdg_window!(HyprCubeApp);
delegate_registry!(HyprCubeApp);

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(
    registry: PanelRegistry,
    preview: PreviewEngine,
    config: AppConfig,
    palette: Palette,
) -> Result<AppConfig, WaylandError> {
    let conn = Connection::connect_to_env().map_err(|_| WaylandError::NoWayland)?;

    let (globals, mut event_queue) = registry_queue_init::<HyprCubeApp>(&conn)
        .map_err(|e| WaylandError::Init(e.to_string()))?;
    let qh = event_queue.handle();

    let compositor_state =
        CompositorState::bind(&globals, &qh).map_err(|e| WaylandError::Init(e.to_string()))?;
    let xdg_shell =
        XdgShell::bind(&globals, &qh).map_err(|e| WaylandError::Init(e.to_string()))?;
    let shm = Shm::bind(&globals, &qh).map_err(|e| WaylandError::Init(e.to_string()))?;

    let surface = compositor_state.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::ServerDefault, &qh);
    window.set_title("HyprCube");
    window.set_app_id("org.hyprcube.HyprCube");
    window.set_min_size(Some((400, 300)));

    let width = config.window_width;
    let height = config.window_height;

    let pool = SlotPool::new((width as usize * height as usize * 4) as usize, &shm)
        .map_err(|e| WaylandError::Init(e.to_string()))?;

    // Initial commit to trigger compositor configure event
    window.wl_surface().commit();

    let mut app = HyprCubeApp {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        _compositor_state: compositor_state,
        _xdg_shell: xdg_shell,
        shm,
        window,
        pool,
        buffer: None,
        registry,
        preview,
        config,
        palette,
        text_renderer: TextRenderer::new(),
        width,
        height,
        needs_redraw: true,
        first_configure: true,
        exit: false,
        form: None,
        form_panel_idx: usize::MAX,
        form_content_width: 0.0,
        pointer: None,
        pointer_x: 0.0,
        pointer_y: 0.0,
        hover_panel: None,
        keyboard: None,
    };

    tracing::info!("wayland window opened ({width}×{height})");

    loop {
        event_queue
            .blocking_dispatch(&mut app)
            .map_err(|e| WaylandError::Dispatch(e.to_string()))?;
        if app.exit {
            tracing::info!("closing window");
            break;
        }
    }

    Ok(app.config)
}
