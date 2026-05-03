use dioxus::prelude::*;
fn app() -> Element {
    rsx! {
        div {
            onscroll: |evt| {
                let _ = evt.get_scroll_offset(); // Check if this exists
            }
        }
    }
}
