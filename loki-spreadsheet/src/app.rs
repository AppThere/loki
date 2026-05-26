// SPDX-License-Identifier: Apache-2.0

//! Root application component for loki-spreadsheet.

use appthere_ui::{use_safe_area, AtThemeContext};
use dioxus::prelude::*;

use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;

/// Root application component.
#[component]
pub fn App() -> Element {
    // Inject the theme context before any shell component renders.
    provide_context(AtThemeContext::default());

    // Open-document tab list. Index 0 of the Vec = document tab 1
    let tabs: Signal<Vec<OpenTab>> = use_signal(Vec::new);
    let active_tab: Signal<usize> = use_signal(|| 0usize); // 0 = Home tab

    // Recent-documents list.
    let recent_docs: Signal<RecentDocuments> = use_signal(RecentDocuments::load);

    provide_context(tabs);
    provide_context(active_tab);
    provide_context(recent_docs);

    let insets = use_safe_area();

    rsx! {
        document::Style {
            "
            html, body, main {{
                margin: 0;
                padding: 0;
                overflow: hidden;
                height: 100%;
            }}
            "
        }

        document::Style {
            "@font-face {{
                font-family: 'Atkinson Hyperlegible Next';
                src: url('dioxus:///assets/fonts/AtkinsonHyperlegibleNext-VF.ttf')
                     format('truetype');
                font-weight: 100 900;
                font-style: normal;
            }}"
        }

        div {
            style: "margin: 0; \
                    padding: {insets.top}px {insets.right}px {insets.bottom}px {insets.left}px; \
                    width: 100vw; height: 100vh; \
                    overflow: hidden; box-sizing: border-box;",
            Router::<Route> {}
        }
    }
}
