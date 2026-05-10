use blitz_traits::events::{BlitzKeyEvent, BlitzMouseButtonEvent, MouseEventButton};
use dioxus_html::{
    geometry::{ClientPoint, ElementPoint, PagePoint, ScreenPoint},
    input_data::{MouseButton, MouseButtonSet},
    point_interaction::{
        InteractionElementOffset, InteractionLocation, ModifiersInteraction, PointerInteraction,
    },
    AnimationData, CancelData, ClipboardData, CompositionData, DragData, FocusData, FormData,
    FormValue, HasFileData, HasFocusData, HasFormData, HasKeyboardData, HasMouseData,
    HasTouchData, HasTouchPointData, HtmlEventConverter, ImageData, KeyboardData, MediaData,
    MountedData, MouseData, PlatformEventData, PointerData, ResizeData, ScrollData, SelectionData,
    ToggleData, TouchData, TouchPoint, TransitionData, VisibleData, WheelData,
};
use keyboard_types::{Code, Key, Location, Modifiers};
use std::any::Any;

pub struct NativeConverter {}

impl HtmlEventConverter for NativeConverter {
    fn convert_cancel_data(&self, _event: &PlatformEventData) -> CancelData {
        unimplemented!("todo: convert_cancel_data in dioxus-native. requires support in blitz")
    }

    fn convert_form_data(&self, event: &PlatformEventData) -> FormData {
        event.downcast::<NativeFormData>().unwrap().clone().into()
    }

    fn convert_mouse_data(&self, event: &PlatformEventData) -> MouseData {
        event.downcast::<NativeClickData>().unwrap().clone().into()
    }

    fn convert_keyboard_data(&self, event: &PlatformEventData) -> KeyboardData {
        event
            .downcast::<BlitzKeyboardData>()
            .unwrap()
            .clone()
            .into()
    }

    fn convert_focus_data(&self, _event: &PlatformEventData) -> FocusData {
        NativeFocusData {}.into()
    }

    fn convert_animation_data(&self, _event: &PlatformEventData) -> AnimationData {
        unimplemented!("todo: convert_animation_data in dioxus-native. requires support in blitz")
    }

    fn convert_clipboard_data(&self, _event: &PlatformEventData) -> ClipboardData {
        unimplemented!("todo: convert_clipboard_data in dioxus-native. requires support in blitz")
    }

    fn convert_composition_data(&self, _event: &PlatformEventData) -> CompositionData {
        unimplemented!("todo: convert_composition_data in dioxus-native. requires support in blitz")
    }

    fn convert_drag_data(&self, _event: &PlatformEventData) -> DragData {
        unimplemented!("todo: convert_drag_data in dioxus-native. requires support in blitz")
    }

    fn convert_image_data(&self, _event: &PlatformEventData) -> ImageData {
        unimplemented!("todo: convert_image_data in dioxus-native. requires support in blitz")
    }

    fn convert_media_data(&self, _event: &PlatformEventData) -> MediaData {
        unimplemented!("todo: convert_media_data in dioxus-native. requires support in blitz")
    }

    fn convert_mounted_data(&self, _event: &PlatformEventData) -> MountedData {
        unimplemented!("todo: convert_mounted_data in dioxus-native. requires support in blitz")
    }

    fn convert_pointer_data(&self, _event: &PlatformEventData) -> PointerData {
        unimplemented!("todo: convert_pointer_data in dioxus-native. requires support in blitz")
    }

    fn convert_scroll_data(&self, _event: &PlatformEventData) -> ScrollData {
        unimplemented!("todo: convert_scroll_data in dioxus-native. requires support in blitz")
    }

    fn convert_selection_data(&self, _event: &PlatformEventData) -> SelectionData {
        unimplemented!("todo: convert_selection_data in dioxus-native. requires support in blitz")
    }

    fn convert_toggle_data(&self, _event: &PlatformEventData) -> ToggleData {
        unimplemented!("todo: convert_toggle_data in dioxus-native. requires support in blitz")
    }

    fn convert_touch_data(&self, event: &PlatformEventData) -> TouchData {
        // Touch events in blitz-shell 0.2.3 are synthesised as mouse events
        // (see patches/blitz-shell). The PlatformEventData wraps a
        // NativeClickData whose coordinates carry the touch position. We
        // extract those coordinates and build a single-point TouchData.
        //
        // TODO(multi-touch): only single touch points are forwarded.
        // Pinch-to-zoom and two-finger gestures require multi-touch support.
        if let Some(click) = event.downcast::<NativeClickData>() {
            let point = NativeTouchPoint {
                client_x: click.inner.x as f64,
                client_y: click.inner.y as f64,
            };
            let touch_data = NativeTouchData {
                touches: vec![point.clone()],
                changed_touches: vec![point.clone()],
                target_touches: vec![point],
                modifiers: click.inner.mods,
            };
            TouchData::new(touch_data)
        } else {
            // No recognisable payload — return a zero-coordinate single-point
            // event rather than panicking, so the handler receives a valid
            // object even if coordinates are unusable.
            let point = NativeTouchPoint { client_x: 0.0, client_y: 0.0 };
            TouchData::new(NativeTouchData {
                touches: vec![point.clone()],
                changed_touches: vec![point.clone()],
                target_touches: vec![point],
                modifiers: Modifiers::default(),
            })
        }
    }

    fn convert_transition_data(&self, _event: &PlatformEventData) -> TransitionData {
        unimplemented!("todo: convert_transition_data in dioxus-native. requires support in blitz")
    }

    fn convert_wheel_data(&self, _event: &PlatformEventData) -> WheelData {
        unimplemented!("todo: convert_wheel_data in dioxus-native. requires support in blitz")
    }

    fn convert_resize_data(&self, _event: &PlatformEventData) -> ResizeData {
        unimplemented!("todo: convert_resize_data in dioxus-native. requires support in blitz")
    }

    fn convert_visible_data(&self, _event: &PlatformEventData) -> VisibleData {
        unimplemented!("todo: convert_visible_data in dioxus-native. requires support in blitz")
    }
}

#[derive(Clone, Debug)]
pub struct NativeFormData {
    pub value: String,
    pub values: Vec<(String, FormValue)>,
}

impl HasFormData for NativeFormData {
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }

    fn value(&self) -> String {
        self.value.clone()
    }

    fn values(&self) -> Vec<(String, FormValue)> {
        self.values.clone()
    }
    fn valid(&self) -> bool {
        // todo: actually implement validation here.
        true
    }
}

impl HasFileData for NativeFormData {
    fn files(&self) -> Vec<dioxus_html::FileData> {
        vec![]
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BlitzKeyboardData(pub(crate) BlitzKeyEvent);

impl ModifiersInteraction for BlitzKeyboardData {
    fn modifiers(&self) -> Modifiers {
        self.0.modifiers
    }
}

impl HasKeyboardData for BlitzKeyboardData {
    fn key(&self) -> Key {
        self.0.key.clone()
    }

    fn code(&self) -> Code {
        self.0.code
    }

    fn location(&self) -> Location {
        self.0.location
    }

    fn is_auto_repeating(&self) -> bool {
        self.0.is_auto_repeating
    }

    fn is_composing(&self) -> bool {
        self.0.is_composing
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn Any
    }
}

#[derive(Clone)]
pub struct NativeClickData {
    pub(crate) inner: BlitzMouseButtonEvent,
    /// Element-local x coordinate (offset from target element's top-left corner).
    /// Computed via blitz-dom `Node::absolute_position` at event dispatch time.
    pub(crate) element_x: f32,
    /// Element-local y coordinate (offset from target element's top-left corner).
    /// Computed via blitz-dom `Node::absolute_position` at event dispatch time.
    pub(crate) element_y: f32,
}

impl InteractionLocation for NativeClickData {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.inner.x as _, self.inner.y as _)
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        unimplemented!()
    }

    fn page_coordinates(&self) -> PagePoint {
        unimplemented!()
    }
}

impl InteractionElementOffset for NativeClickData {
    fn element_coordinates(&self) -> ElementPoint {
        ElementPoint::new(self.element_x as _, self.element_y as _)
    }
}

impl ModifiersInteraction for NativeClickData {
    fn modifiers(&self) -> Modifiers {
        self.inner.mods
    }
}

impl PointerInteraction for NativeClickData {
    fn trigger_button(&self) -> Option<MouseButton> {
        Some(match self.inner.button {
            MouseEventButton::Main => MouseButton::Primary,
            MouseEventButton::Auxiliary => MouseButton::Auxiliary,
            MouseEventButton::Secondary => MouseButton::Secondary,
            MouseEventButton::Fourth => MouseButton::Fourth,
            MouseEventButton::Fifth => MouseButton::Fifth,
        })
    }

    fn held_buttons(&self) -> MouseButtonSet {
        dioxus_html::input_data::decode_mouse_button_set(self.inner.buttons.bits() as u16)
    }
}
impl HasMouseData for NativeClickData {
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}

#[derive(Clone)]
pub struct NativeFocusData {}
impl HasFocusData for NativeFocusData {
    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}

// ── Touch data types ──────────────────────────────────────────────────────────

/// A single touch contact point, carrying client-coordinate position.
#[derive(Clone, Debug)]
struct NativeTouchPoint {
    client_x: f64,
    client_y: f64,
}

impl InteractionLocation for NativeTouchPoint {
    fn client_coordinates(&self) -> ClientPoint {
        ClientPoint::new(self.client_x, self.client_y)
    }

    fn screen_coordinates(&self) -> ScreenPoint {
        // Screen coordinates are not available through the synthesised mouse
        // event path; return the client coordinates as a reasonable fallback.
        ScreenPoint::new(self.client_x, self.client_y)
    }

    fn page_coordinates(&self) -> PagePoint {
        PagePoint::new(self.client_x, self.client_y)
    }
}

impl HasTouchPointData for NativeTouchPoint {
    fn identifier(&self) -> i32 {
        0
    }

    fn force(&self) -> f64 {
        1.0
    }

    fn radius(&self) -> ScreenPoint {
        ScreenPoint::new(1.0, 1.0)
    }

    fn rotation(&self) -> f64 {
        0.0
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

/// Touch event data for a single synthesised touch contact.
#[derive(Clone, Debug)]
struct NativeTouchData {
    touches: Vec<NativeTouchPoint>,
    changed_touches: Vec<NativeTouchPoint>,
    target_touches: Vec<NativeTouchPoint>,
    modifiers: Modifiers,
}

impl ModifiersInteraction for NativeTouchData {
    fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

impl HasTouchData for NativeTouchData {
    fn touches(&self) -> Vec<TouchPoint> {
        self.touches.iter().cloned().map(TouchPoint::new).collect()
    }

    fn touches_changed(&self) -> Vec<TouchPoint> {
        self.changed_touches.iter().cloned().map(TouchPoint::new).collect()
    }

    fn target_touches(&self) -> Vec<TouchPoint> {
        self.target_touches.iter().cloned().map(TouchPoint::new).collect()
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}
