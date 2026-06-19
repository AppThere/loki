use crate::BlitzShellProvider;
use crate::convert_events::{
    color_scheme_to_theme, theme_to_color_scheme, winit_ime_to_blitz, winit_key_event_to_blitz,
    winit_modifiers_to_kbt_modifiers,
};
use crate::event::{BlitzShellEvent, create_waker};
use anyrender::WindowRenderer;
use blitz_dom::Document;
use blitz_paint::paint_scene;
use blitz_traits::events::{
    BlitzKeyEvent, BlitzMouseButtonEvent, KeyState, MouseEventButton, MouseEventButtons, UiEvent,
};
use blitz_traits::shell::Viewport;
use winit::keyboard::PhysicalKey;

use std::sync::Arc;
use std::task::Waker;
use std::time::Instant;
use winit::event::{ElementState, MouseButton, TouchPhase};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::{Theme, WindowAttributes, WindowId};
use winit::{event::Modifiers, event::WindowEvent, keyboard::KeyCode, window::Window};

/// Minimum movement in logical pixels before a touch is treated as a scroll rather than a tap.
const TOUCH_SLOP_PX: f64 = 8.0;

/// Duration after which a stationary touch is classified as a long press.
const LONG_PRESS_DURATION: std::time::Duration = std::time::Duration::from_millis(500);

/// State tracked for an in-progress touch contact.
#[derive(Debug, Clone)]
pub struct TouchState {
    /// OS-assigned identifier for this touch contact.
    pub id: u64,
    /// Window-logical-pixel position where the touch started.
    pub start_pos: (f64, f64),
    /// Monotonic time when the touch started (for long-press detection).
    pub start_time: Instant,
}

#[cfg(feature = "accessibility")]
use crate::accessibility::AccessibilityState;

pub struct WindowConfig<Rend: WindowRenderer> {
    doc: Box<dyn Document>,
    attributes: WindowAttributes,
    renderer: Rend,
}

impl<Rend: WindowRenderer> WindowConfig<Rend> {
    pub fn new(doc: Box<dyn Document>, renderer: Rend) -> Self {
        Self::with_attributes(doc, renderer, Window::default_attributes())
    }

    pub fn with_attributes(
        doc: Box<dyn Document>,
        renderer: Rend,
        attributes: WindowAttributes,
    ) -> Self {
        WindowConfig {
            doc,
            attributes,
            renderer,
        }
    }
}

pub struct View<Rend: WindowRenderer> {
    pub doc: Box<dyn Document>,

    pub renderer: Rend,
    pub waker: Option<Waker>,

    pub event_loop_proxy: EventLoopProxy<BlitzShellEvent>,
    pub window: Arc<Window>,

    /// The state of the keyboard modifiers (ctrl, shift, etc). Winit/Tao don't track these for us so we
    /// need to store them in order to have access to them when processing keypress events
    pub theme_override: Option<Theme>,
    pub keyboard_modifiers: Modifiers,
    pub buttons: MouseEventButtons,
    pub mouse_pos: (f32, f32),
    pub animation_timer: Option<Instant>,
    pub is_visible: bool,

    /// Whether the on-screen keyboard / IME is currently requested.
    /// Mirrors the last value passed to `Window::set_ime_allowed` so we only
    /// call into the (JNI-backed, on Android) winit IME path on real changes.
    pub ime_active: bool,

    /// In-progress touch contact for tap/scroll/long-press classification.
    pub touch_start: Option<TouchState>,
    /// Last logical-pixel position during an active scroll gesture.
    /// `Some` while the finger is scrolling; `None` otherwise.
    pub touch_scroll_last_pos: Option<(f64, f64)>,

    #[cfg(feature = "accessibility")]
    /// Accessibility adapter for `accesskit`.
    pub accessibility: AccessibilityState,
}

impl<Rend: WindowRenderer> View<Rend> {
    pub fn init(
        config: WindowConfig<Rend>,
        event_loop: &ActiveEventLoop,
        proxy: &EventLoopProxy<BlitzShellEvent>,
    ) -> Self {
        let winit_window = Arc::from(event_loop.create_window(config.attributes).unwrap());

        // Start with the IME disabled and let focus drive it
        // (`update_ime_for_focus`).  On Android `set_ime_allowed(true)` calls
        // `AndroidApp::show_soft_input`, so leaving it on here would pop the
        // soft keyboard at launch (and the call is a no-op before the window is
        // focused anyway).  We raise it only when a text-editing element — one
        // carrying `inputmode` (≠ "none"), or an `<input>`/`<textarea>` — gains
        // focus, and lower it when focus leaves.
        winit_window.set_ime_allowed(false);

        // Create viewport
        let size = winit_window.inner_size();
        let scale = winit_window.scale_factor() as f32;
        let theme = winit_window.theme().unwrap_or(Theme::Light);
        let color_scheme = theme_to_color_scheme(theme);
        let viewport = Viewport::new(size.width, size.height, scale, color_scheme);

        // Create shell provider
        let shell_provider = BlitzShellProvider::new(winit_window.clone());

        let mut doc = config.doc;
        doc.set_viewport(viewport);
        doc.set_shell_provider(Arc::new(shell_provider));

        // If the document title is set prior to the window being created then it will
        // have been sent to a dummy ShellProvider and won't get picked up.
        // So we look for it here and set it if present.
        let title = doc.find_title_node().map(|node| node.text_content());
        if let Some(title) = title {
            winit_window.set_title(&title);
        }

        Self {
            renderer: config.renderer,
            waker: None,
            animation_timer: None,
            keyboard_modifiers: Default::default(),
            event_loop_proxy: proxy.clone(),
            window: winit_window.clone(),
            doc,
            theme_override: None,
            buttons: MouseEventButtons::None,
            mouse_pos: Default::default(),
            is_visible: winit_window.is_visible().unwrap_or(true),
            ime_active: false,
            touch_start: None,
            touch_scroll_last_pos: None,
            #[cfg(feature = "accessibility")]
            accessibility: AccessibilityState::new(&winit_window, proxy.clone()),
        }
    }

    pub fn replace_document(&mut self, new_doc: Box<dyn Document>, retain_scroll_position: bool) {
        let scroll = self.doc.viewport_scroll();
        let viewport = self.doc.viewport().clone();
        let shell_provider = self.doc.shell_provider.clone();

        self.doc = new_doc;
        self.doc.set_viewport(viewport);
        self.doc.set_shell_provider(shell_provider);
        self.poll();
        self.request_redraw();

        if retain_scroll_position {
            self.doc.set_viewport_scroll(scroll);
        }
    }

    pub fn theme_override(&self) -> Option<Theme> {
        self.theme_override
    }

    pub fn current_theme(&self) -> Theme {
        color_scheme_to_theme(self.doc.viewport().color_scheme)
    }

    pub fn set_theme_override(&mut self, theme: Option<Theme>) {
        self.theme_override = theme;
        let theme = theme.or(self.window.theme()).unwrap_or(Theme::Light);
        self.with_viewport(|v| v.color_scheme = theme_to_color_scheme(theme));
    }

    pub fn downcast_doc_mut<T: 'static>(&mut self) -> &mut T {
        self.doc.as_any_mut().downcast_mut::<T>().unwrap()
    }

    pub fn current_animation_time(&mut self) -> f64 {
        match &self.animation_timer {
            Some(start) => Instant::now().duration_since(*start).as_secs_f64(),
            None => {
                self.animation_timer = Some(Instant::now());
                0.0
            }
        }
    }
}

impl<Rend: WindowRenderer> View<Rend> {
    pub fn resume(&mut self) {
        // Resolve dom
        let animation_time = self.current_animation_time();
        self.doc.resolve(animation_time);

        // On Android, inner_size() reads from the ANativeWindow buffer dimensions
        // which are zero until the first WindowResized event fires (the window
        // object exists after InitWindow/Resumed, but its buffer size is set later
        // by onNativeWindowChanged → WindowResized).  Calling renderer.resume()
        // with (0,0) would configure a wgpu surface of zero size and the Stylo
        // device would compute 100vh = 0, collapsing all elements to zero height.
        // Defer renderer activation to handle_winit_event(Resized) in that case.
        let actual = self.window.inner_size();
        log::info!("[LOKI/resume] inner_size=({},{})", actual.width, actual.height);

        if actual.width == 0 || actual.height == 0 {
            log::info!("[LOKI/resume] zero dims — deferring renderer to Resized");
            self.waker = Some(create_waker(&self.event_loop_proxy, self.window_id()));
            return;
        }

        // Correct stored viewport if it diverges from the actual window size
        let (stored_w, stored_h) = self.doc.viewport().window_size;
        if stored_w != actual.width || stored_h != actual.height {
            log::info!("[LOKI/resume] correcting viewport ({stored_w},{stored_h}) → ({},{})", actual.width, actual.height);
            {
                let mut vp = self.doc.viewport_mut();
                vp.window_size = (actual.width, actual.height);
            }
            self.doc.resolve(animation_time);
        }

        let (width, height) = self.doc.viewport().window_size;
        let scale = self.doc.viewport().scale_f64();
        log::info!("[LOKI/resume] activating renderer ({width},{height}) scale={scale}");

        // Resume renderer
        self.renderer.resume(self.window.clone(), width, height);
        if !self.renderer.is_active() {
            panic!("Renderer failed to resume");
        };

        log::info!("[LOKI/resume] calling render");
        self.renderer
            .render(|scene| paint_scene(scene, &self.doc, scale, width, height));
        log::info!("[LOKI/resume] render done");

        // Set waker
        self.waker = Some(create_waker(&self.event_loop_proxy, self.window_id()));
    }

    /// Activate the renderer for the first time using real surface dimensions.
    ///
    /// Called from handle_winit_event(Resized) when renderer.resume() was
    /// deferred because inner_size() was (0,0) during the initial resumed() call.
    fn activate_renderer(&mut self, width: u32, height: u32) {
        let animation_time = self.current_animation_time();
        log::info!("[LOKI/activate] activating renderer ({width},{height})");
        {
            let mut vp = self.doc.viewport_mut();
            vp.window_size = (width, height);
        }
        self.doc.resolve(animation_time);
        let scale = self.doc.viewport().scale_f64();
        self.renderer.resume(self.window.clone(), width, height);
        if !self.renderer.is_active() {
            log::info!("[LOKI/activate] renderer.resume() failed");
            return;
        }
        log::info!("[LOKI/activate] rendering initial frame");
        self.renderer
            .render(|scene| paint_scene(scene, &self.doc, scale, width, height));
        // Request another redraw so any pending CSS/DOM changes (applied while
        // the renderer was inactive) are also painted.
        self.request_redraw();
    }

    pub fn suspend(&mut self) {
        self.waker = None;
        self.renderer.suspend();
    }

    pub fn poll(&mut self) -> bool {
        if let Some(waker) = &self.waker {
            let cx = std::task::Context::from_waker(waker);
            if self.doc.poll(Some(cx)) {
                #[cfg(feature = "accessibility")]
                {
                    if self.doc.has_changes() {
                        self.accessibility.update_tree(&self.doc);
                    }
                }

                self.request_redraw();
                return true;
            }
        }

        false
    }

    pub fn request_redraw(&self) {
        if self.renderer.is_active() {
            self.window.request_redraw();
        }
    }

    pub fn redraw(&mut self) {
        let animation_time = self.current_animation_time();
        self.doc.resolve(animation_time);
        let (width, height) = self.doc.viewport().window_size;
        // On Windows, minimising the window fires Resized(0,0), which sets the
        // viewport to (0,0).  Poll events still run and can trigger a redraw;
        // calling renderer.render() with zero dimensions panics inside WGPU.
        // Skip the render entirely — the window is not visible anyway.
        if width == 0 || height == 0 {
            return;
        }
        let scale = self.doc.viewport().scale_f64();
        log::info!("[LOKI/redraw] viewport=({width},{height}) scale={scale}");
        self.renderer
            .render(|scene| paint_scene(scene, &self.doc, scale, width, height));

        if self.is_visible && self.doc.is_animating() {
            self.request_redraw();
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    #[inline]
    pub fn with_viewport(&mut self, cb: impl FnOnce(&mut Viewport)) {
        let mut viewport = self.doc.viewport_mut();
        cb(&mut viewport);
        drop(viewport);
        let (width, height) = self.doc.viewport().window_size;
        if width > 0 && height > 0 {
            self.renderer.set_size(width, height);
            self.request_redraw();
        }
    }

    #[cfg(feature = "accessibility")]
    pub fn build_accessibility_tree(&mut self) {
        self.accessibility.update_tree(&self.doc);
    }

    /// PATCH(loki): re-dispatch `onscroll` to every scroll container with its
    /// fresh client geometry.  This lets reactive embedders (the editor's
    /// width-driven reflow / view-mode default) react to a window resize, to the
    /// first real size on Android, and to a scroll container that mounted after
    /// the initial layout (e.g. once an async document load completes) — all
    /// without requiring the user to scroll first.
    pub fn resync_scroll_geometry(&mut self) {
        let (w, h) = self.doc.viewport().window_size;
        if w == 0 || h == 0 {
            return;
        }
        // Resolve so each scroll container's `final_layout` reflects the new
        // viewport before its geometry is read for the scroll event.
        let time = self.current_animation_time();
        self.doc.resolve(time);
        let mut changes = Vec::new();
        self.doc.collect_scroll_containers(&mut changes);
        if !changes.is_empty() {
            self.doc.handle_scroll_changes(&changes);
            self.request_redraw();
        }
    }

    /// Returns `true` when the currently-focused DOM node is a text-editing
    /// surface that should summon the platform soft keyboard / IME.
    ///
    /// PATCH(loki): blitz-dom focuses any element with `tabindex`/`<button>` on
    /// click, but only some of those accept text.  We treat a node as a text
    /// target when it is an `<input>` (of a textual type) / `<textarea>`, or
    /// when it carries an `inputmode` attribute whose value is not `"none"`.
    /// The Loki editor canvas is a focusable `<div inputmode="text">`, so a tap
    /// on it raises the keyboard while a tap on a ribbon `<button>` does not.
    fn focused_node_wants_ime(&self) -> bool {
        let Some(node_id) = self.doc.get_focussed_node_id() else {
            return false;
        };
        let Some(node) = self.doc.get_node(node_id) else {
            return false;
        };
        let Some(el) = node.data.downcast_element() else {
            return false;
        };

        let attr = |name: &str| {
            el.attrs
                .iter()
                .find(|a| a.name.local.as_ref() == name)
                .map(|a| a.value.as_str())
        };

        match el.name.local.as_ref() {
            "textarea" => true,
            "input" => !matches!(
                attr("type"),
                Some("button" | "submit" | "reset" | "checkbox" | "radio" | "file" | "hidden")
            ),
            // Any element may opt in to the keyboard via the standard inputmode
            // hint (the editor canvas does); `inputmode="none"` opts out.
            _ => matches!(attr("inputmode"), Some(mode) if mode != "none"),
        }
    }

    /// Returns `true` when the focused node is a Blitz-native text field
    /// (`<input>` / `<textarea>`), which has its own IME handling and must not
    /// receive synthetic keydown events.
    fn focused_is_native_text_input(&self) -> bool {
        self.doc
            .get_focussed_node_id()
            .and_then(|id| self.doc.get_node(id))
            .and_then(|node| node.data.downcast_element())
            .is_some_and(|el| matches!(el.name.local.as_ref(), "input" | "textarea"))
    }

    /// Dispatch `text` to the focused node as a synthetic key press/release pair
    /// carrying the whole string as `Key::Character`.  Used to deliver committed
    /// IME / soft-keyboard text to the custom editor canvas, whose `onkeydown`
    /// handler inserts `Key::Character(_)` payloads verbatim.
    fn dispatch_synthetic_text(&mut self, text: &str) {
        let base = BlitzKeyEvent {
            key: keyboard_types::Key::Character(text.to_string()),
            code: keyboard_types::Code::Unidentified,
            modifiers: keyboard_types::Modifiers::default(),
            location: keyboard_types::Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: Some(text.into()),
        };
        self.doc.handle_ui_event(UiEvent::KeyDown(base.clone()));
        self.doc.handle_ui_event(UiEvent::KeyUp(BlitzKeyEvent {
            state: KeyState::Released,
            ..base
        }));
    }

    /// Sync the platform IME / soft keyboard to the current focus, calling into
    /// winit (and thus `AndroidApp::show/hide_soft_input` on Android) only when
    /// the desired state actually changes.
    fn update_ime_for_focus(&mut self) {
        let wants_ime = self.focused_node_wants_ime();
        if wants_ime != self.ime_active {
            self.ime_active = wants_ime;
            self.window.set_ime_allowed(wants_ime);
        }
    }

    pub fn handle_winit_event(&mut self, event: WindowEvent) {
        match event {
            // Window lifecycle events
            WindowEvent::Destroyed => {}
            WindowEvent::ActivationTokenDone { .. } => {},
            WindowEvent::CloseRequested => {
                // Currently handled at the level above in application.rs
            }
            WindowEvent::RedrawRequested => {
                log::info!("[LOKI/event] RedrawRequested");
                self.redraw();
            }

            // Window size/position events
            WindowEvent::Moved(_) => {}
            WindowEvent::Occluded(is_occluded) => {
                self.is_visible = !is_occluded;
                if self.is_visible {
                    self.request_redraw();
                }
            },
            WindowEvent::Resized(physical_size) => {
                log::info!("[LOKI/event] Resized({},{}) renderer_active={}", physical_size.width, physical_size.height, self.renderer.is_active());
                if !self.renderer.is_active()
                    && physical_size.width > 0
                    && physical_size.height > 0
                {
                    // Renderer was deferred because inner_size() was (0,0) at resume time.
                    self.activate_renderer(physical_size.width, physical_size.height);
                } else {
                    self.with_viewport(|v| v.window_size = (physical_size.width, physical_size.height));
                }
                // Re-emit onscroll with the new client size so width-reactive
                // embedders (reflow layout, view-mode default) update on resize
                // and on the first real Android size.
                if physical_size.width > 0 && physical_size.height > 0 {
                    self.resync_scroll_geometry();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.with_viewport(|v| v.set_hidpi_scale(scale_factor as f32));
            }

            // Theme events
            WindowEvent::ThemeChanged(theme) => {
                let color_scheme = theme_to_color_scheme(self.theme_override.unwrap_or(theme));
                self.doc.viewport_mut().color_scheme = color_scheme;
            }

            // Text / keyboard events
            WindowEvent::Ime(ime_event) => {
                // PATCH(loki): route committed IME text from a custom editing
                // surface (the Loki canvas — an `inputmode` element, not a Blitz
                // TextInput) into the focused node as a synthetic keydown so the
                // existing `onkeydown` insertion path handles it.  This is what
                // makes the Android soft keyboard actually type into the canvas.
                // Real `<input>`/`<textarea>` keep Blitz's native IME handling.
                if let winit::event::Ime::Commit(text) = &ime_event {
                    if !text.is_empty()
                        && self.focused_node_wants_ime()
                        && !self.focused_is_native_text_input()
                    {
                        self.dispatch_synthetic_text(text);
                        self.request_redraw();
                        return;
                    }
                }
                self.doc.handle_ui_event(UiEvent::Ime(winit_ime_to_blitz(ime_event)));
                self.request_redraw();
            },
            WindowEvent::ModifiersChanged(new_state) => {
                // Store new keyboard modifier (ctrl, shift, etc) state for later use
                self.keyboard_modifiers = new_state;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(key_code) = event.physical_key else {
                    return;
                };

                if event.state.is_pressed() {
                    let ctrl = self.keyboard_modifiers.state().control_key();
                    let meta = self.keyboard_modifiers.state().super_key();
                    let alt = self.keyboard_modifiers.state().alt_key();

                    // Ctrl/Super keyboard shortcuts
                    if ctrl | meta {
                        match key_code {
                            KeyCode::Equal => self.doc.viewport_mut().zoom_by(0.1),
                            KeyCode::Minus => self.doc.viewport_mut().zoom_by(-0.1),
                            KeyCode::Digit0 => self.doc.viewport_mut().set_zoom(1.0),
                            _ => {}
                        };
                    }

                    // Alt keyboard shortcuts
                    if alt {
                        match key_code {
                            KeyCode::KeyD => {
                                self.doc.devtools_mut().toggle_show_layout();
                                self.request_redraw();
                            }
                            KeyCode::KeyH => {
                                self.doc.devtools_mut().toggle_highlight_hover();
                                self.request_redraw();
                            }
                            KeyCode::KeyT => self.doc.print_taffy_tree(),
                            _ => {}
                        };
                    }

                }

                // Unmodified keypresses
                let key_event_data = winit_key_event_to_blitz(&event, self.keyboard_modifiers.state());
                let event = if event.state.is_pressed() {
                    UiEvent::KeyDown(key_event_data)
                } else {
                    UiEvent::KeyUp(key_event_data)
                };

                self.doc.handle_ui_event(event);
                // Tab / Shift-Tab can move focus between fields — keep IME in sync.
                self.update_ime_for_focus();
                self.request_redraw();
            }


            // Mouse/pointer events
            WindowEvent::CursorEntered { /*device_id*/.. } => {}
            WindowEvent::CursorLeft { /*device_id*/.. } => {}
            WindowEvent::CursorMoved { position, .. } => {
                let winit::dpi::LogicalPosition::<f32> { x, y } = position.to_logical(self.window.scale_factor());
                self.mouse_pos = (x, y);
                let event = UiEvent::MouseMove(BlitzMouseButtonEvent {
                    x,
                    y,
                    button: Default::default(),
                    buttons: self.buttons,
                    mods: winit_modifiers_to_kbt_modifiers(self.keyboard_modifiers.state()),
                });
                self.doc.handle_ui_event(event);
            }
            WindowEvent::MouseInput { button, state, .. } => {
                let button = match button {
                    MouseButton::Left => MouseEventButton::Main,
                    MouseButton::Right => MouseEventButton::Secondary,
                    _ => return,
                };

                match state {
                    ElementState::Pressed => self.buttons |= button.into(),
                    ElementState::Released => self.buttons ^= button.into(),
                }

                let event = BlitzMouseButtonEvent {
                    x: self.mouse_pos.0,
                    y: self.mouse_pos.1,
                    button,
                    buttons: self.buttons,
                    mods: winit_modifiers_to_kbt_modifiers(self.keyboard_modifiers.state()),
                };

                let event = match state {
                    ElementState::Pressed => UiEvent::MouseDown(event),
                    ElementState::Released => UiEvent::MouseUp(event),
                };
                self.doc.handle_ui_event(event);
                // Focus is assigned on click (MouseUp); sync the soft keyboard.
                self.update_ime_for_focus();
                self.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (scroll_x, scroll_y)= match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (x as f64 * 20.0, y as f64 * 20.0),
                    winit::event::MouseScrollDelta::PixelDelta(offsets) => (offsets.x, offsets.y)
                };

                // PATCH(loki): collect per-node scroll changes and forward them
                // to the embedder so Dioxus `onscroll` handlers fire.
                let hover = self.doc.get_hover_node_id();
                let mut changes = Vec::new();
                let has_changed = if let Some(hover_node_id) = hover {
                    self.doc
                        .scroll_node_by_collect(hover_node_id, scroll_x, scroll_y, &mut changes)
                } else {
                    self.doc.scroll_viewport_by_has_changed(scroll_x, scroll_y)
                };
                // DIAG(loki-scroll): temporary — remove once the frozen-thumb
                // regression is fixed. Reports whether the wheel found a hover
                // node and whether any scroll-container change was collected to
                // dispatch `onscroll`.
                eprintln!(
                    "[loki-scroll-diag] wheel hover={hover:?} collected={} changed={has_changed}",
                    changes.len()
                );
                if !changes.is_empty() {
                    self.doc.handle_scroll_changes(&changes);
                }

                if has_changed {
                    self.request_redraw();
                }
            }

            // File events
            WindowEvent::DroppedFile(_) => {}
            WindowEvent::HoveredFile(_) => {}
            WindowEvent::HoveredFileCancelled => {}
            WindowEvent::Focused(_) => {}

            // Touch and motion events
            // Upstream blitz-shell 0.2.3 discards all touch events. This patch
            // synthesises them as mouse events so that Dioxus ontouchstart /
            // ontouchmove / ontouchend handlers (which are wired to mouse events
            // through blitz-dom) receive the correct coordinates. Native
            // UiEvent::Touch* variants do not exist in blitz-traits 0.2.x —
            // synthesis is the only available path.
            WindowEvent::Touch(touch) => {
                let scale = self.window.scale_factor();
                let logical = touch.location.to_logical::<f64>(scale);
                let (lx, ly) = (logical.x as f32, logical.y as f32);

                match touch.phase {
                    TouchPhase::Started => {
                        // Synthesise CursorMoved then MouseDown so that blitz-dom
                        // performs a fresh hit-test at the touch position before
                        // dispatching the press event.
                        self.mouse_pos = (lx, ly);
                        self.doc.handle_ui_event(UiEvent::MouseMove(BlitzMouseButtonEvent {
                            x: lx,
                            y: ly,
                            button: MouseEventButton::Main,
                            buttons: MouseEventButtons::Primary,
                            mods: winit_modifiers_to_kbt_modifiers(
                                self.keyboard_modifiers.state(),
                            ),
                        }));
                        self.buttons |= MouseEventButton::Main.into();
                        self.doc.handle_ui_event(UiEvent::MouseDown(BlitzMouseButtonEvent {
                            x: lx,
                            y: ly,
                            button: MouseEventButton::Main,
                            buttons: self.buttons,
                            mods: winit_modifiers_to_kbt_modifiers(
                                self.keyboard_modifiers.state(),
                            ),
                        }));
                        self.touch_start = Some(TouchState {
                            id: touch.id,
                            start_pos: (logical.x, logical.y),
                            start_time: Instant::now(),
                        });
                        self.touch_scroll_last_pos = None;
                        self.request_redraw();
                    }
                    TouchPhase::Moved => {
                        self.mouse_pos = (lx, ly);

                        // Classify the gesture once it moves beyond the slop
                        // threshold or exceeds the long-press hold time.
                        if let Some(ref ts) = self.touch_start {
                            let dx = logical.x - ts.start_pos.0;
                            let dy = logical.y - ts.start_pos.1;
                            if dx.hypot(dy) > TOUCH_SLOP_PX {
                                // Finger moved — it's a scroll gesture.
                                self.touch_start = None;
                                self.touch_scroll_last_pos = Some((logical.x, logical.y));
                            } else if ts.start_time.elapsed() >= LONG_PRESS_DURATION {
                                // Held in place — long-press, not a scroll.
                                self.touch_start = None;
                            }
                        }

                        if self.touch_scroll_last_pos.is_none() {
                            // Not yet scrolling — forward as mouse move so that
                            // Dioxus ontouchmove / long-press handlers fire.
                            self.doc.handle_ui_event(UiEvent::MouseMove(BlitzMouseButtonEvent {
                                x: lx,
                                y: ly,
                                button: MouseEventButton::Main,
                                buttons: self.buttons,
                                mods: winit_modifiers_to_kbt_modifiers(
                                    self.keyboard_modifiers.state(),
                                ),
                            }));
                        }

                        // Drive CSS overflow scroll when a scroll gesture is active.
                        if let Some(last) = self.touch_scroll_last_pos {
                            let scroll_x = logical.x - last.0;
                            let scroll_y = logical.y - last.1;
                            self.touch_scroll_last_pos = Some((logical.x, logical.y));
                            // PATCH(loki): collect per-node scroll changes and
                            // forward them so Dioxus `onscroll` handlers fire.
                            let mut changes = Vec::new();
                            let changed = if let Some(id) = self.doc.get_hover_node_id() {
                                self.doc
                                    .scroll_node_by_collect(id, scroll_x, scroll_y, &mut changes)
                            } else {
                                self.doc.scroll_viewport_by_has_changed(scroll_x, scroll_y)
                            };
                            if !changes.is_empty() {
                                self.doc.handle_scroll_changes(&changes);
                            }
                            if changed {
                                self.request_redraw();
                                return; // redraw already requested above
                            }
                        }

                        self.request_redraw();
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        // Check before clearing: was this a scroll gesture?
                        let was_scroll = self.touch_scroll_last_pos.is_some();
                        self.touch_scroll_last_pos = None;
                        self.touch_start = None;
                        self.buttons ^= MouseEventButton::Main.into();
                        // Synthesise MouseUp only for tap gestures.  Emitting
                        // MouseUp after a scroll would fire onclick on the element
                        // under the finger when the user lifts off at the end of
                        // a scroll, causing unintended activations.
                        if !was_scroll {
                            self.doc.handle_ui_event(UiEvent::MouseUp(BlitzMouseButtonEvent {
                                x: lx,
                                y: ly,
                                button: MouseEventButton::Main,
                                buttons: self.buttons,
                                mods: winit_modifiers_to_kbt_modifiers(
                                    self.keyboard_modifiers.state(),
                                ),
                            }));
                            // A tap assigns focus (or clears it); raise or drop
                            // the Android soft keyboard accordingly.  Skipped for
                            // scroll gestures, which never change focus.
                            self.update_ime_for_focus();
                        }
                        self.request_redraw();
                    }
                }
            }
            WindowEvent::TouchpadPressure { .. } => {}
            WindowEvent::AxisMotion { .. } => {}
            WindowEvent::PinchGesture { .. } => {},
            WindowEvent::PanGesture { .. } => {},
            WindowEvent::DoubleTapGesture { .. } => {},
            WindowEvent::RotationGesture { .. } => {},
        }
    }
}
