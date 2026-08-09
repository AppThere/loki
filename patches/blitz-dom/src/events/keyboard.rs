use crate::{
    BaseDocument,
    node::{TextBrush, TextInputData},
};
use blitz_traits::{
    events::{BlitzInputEvent, BlitzKeyEvent, DomEvent, DomEventData},
    shell::ShellProvider,
};
use keyboard_types::{Key, Modifiers};
use markup5ever::local_name;
use parley::{FontContext, LayoutContext};

// TODO: support keypress events
enum GeneratedEvent {
    Input,
    Submit,
}

pub(crate) fn handle_keypress<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    target: usize,
    event: BlitzKeyEvent,
    mut dispatch_event: F,
) {
    if event.key == Key::Tab {
        doc.focus_next_node();
        return;
    }

    if let Some(node_id) = doc.focus_node_id {
        if target != node_id {
            return;
        }

        // PATCH(loki): Enter and Space activate a focused non-text element.
        //
        // Without this, `handle_keypress` does nothing at all for anything that
        // is not a text input — so a focused `button` could be *reached* by Tab
        // and never *pressed*, and every control in an embedder's UI was
        // pointer-only. Tab traversal already worked, which is what made the gap
        // hard to see: focus visibly moves, and then the keyboard stops.
        //
        // Dispatched as a synthetic click rather than as a new event kind, so a
        // handler written for the pointer is the handler the keyboard runs —
        // one activation path, which is what makes "the keyboard does what the
        // mouse does" a property of the DOM rather than of each embedder.
        //
        // **A `DomEvent`, not a call to `handle_click`.** `label` forwards to
        // `handle_click` directly (see `events::mouse`), and copying that here
        // is the mistake this comment exists to stop: `handle_click` is the
        // *default action* for a click the embedder has already been told about,
        // so calling it runs the built-in behaviour and the embedder's own
        // `onclick` never fires. Going through `dispatch_event` puts the click
        // back on the driver's queue, where the handler sees it first — which is
        // the whole point, since in a Dioxus app every button's behaviour lives
        // in an `onclick`.
        //
        // Text inputs are excluded because Enter means newline-or-submit and
        // Space means a space there; that is decided below, and the check is
        // `is_text_input` rather than the element name so a `contenteditable`
        // host is covered too.
        if is_activation_key(&event.key) && event.state.is_pressed() && !is_text_input(doc, node_id)
        {
            let data = doc.nodes[node_id].synthetic_click_event(event.modifiers);
            dispatch_event(DomEvent::new(node_id, data));
            return;
        }

        let node = &mut doc.nodes[node_id];
        let Some(element_data) = node.element_data_mut() else {
            return;
        };

        if let Some(input_data) = element_data.text_input_data_mut() {
            let generated_event = apply_keypress_event(
                input_data,
                &mut doc.font_ctx.lock().unwrap(),
                &mut doc.layout_ctx,
                &*doc.shell_provider,
                event,
            );

            if let Some(generated_event) = generated_event {
                match generated_event {
                    GeneratedEvent::Input => {
                        let value = input_data.editor.raw_text().to_string();
                        dispatch_event(DomEvent::new(
                            node_id,
                            DomEventData::Input(BlitzInputEvent { value }),
                        ));
                    }
                    GeneratedEvent::Submit => {
                        // TODO: Generate submit event that can be handled by script
                        implicit_form_submission(doc, target);
                    }
                }
            }
        }
    }
}

/// PATCH(loki): the two keys that activate a control, per the HTML activation
/// behaviour every platform's accessibility guidance assumes.
///
/// `Space` arrives as `Character(" ")` rather than as a named key — the same
/// translation trap an embedder hits, and the reason this is a named predicate
/// rather than a literal in the `if`.
fn is_activation_key(key: &Key) -> bool {
    match key {
        Key::Enter => true,
        Key::Character(c) => c == " ",
        _ => false,
    }
}

/// PATCH(loki): whether the node edits text, and so owns Enter and Space itself.
fn is_text_input(doc: &BaseDocument, node_id: usize) -> bool {
    doc.nodes[node_id]
        .data
        .downcast_element()
        .is_some_and(|el| el.text_input_data().is_some())
}

#[cfg(target_os = "macos")]
const ACTION_MOD: Modifiers = Modifiers::SUPER;
#[cfg(not(target_os = "macos"))]
const ACTION_MOD: Modifiers = Modifiers::CONTROL;

fn apply_keypress_event(
    input_data: &mut TextInputData,
    font_ctx: &mut FontContext,
    layout_ctx: &mut LayoutContext<TextBrush>,
    shell_provider: &dyn ShellProvider,
    event: BlitzKeyEvent,
) -> Option<GeneratedEvent> {
    // Do nothing if it is a keyup event
    if !event.state.is_pressed() {
        return None;
    }

    let mods = event.modifiers;
    let shift = mods.contains(Modifiers::SHIFT);
    let action_mod = mods.contains(ACTION_MOD);

    let is_multiline = input_data.is_multiline;
    let editor = &mut input_data.editor;
    let mut driver = editor.driver(font_ctx, layout_ctx);
    match event.key {
        Key::Character(c) if action_mod && matches!(c.as_str(), "c" | "x" | "v") => {
            match c.to_lowercase().as_str() {
                "c" => {
                    if let Some(text) = driver.editor.selected_text() {
                        let _ = shell_provider.set_clipboard_text(text.to_owned());
                    }
                }
                "x" => {
                    if let Some(text) = driver.editor.selected_text() {
                        let _ = shell_provider.set_clipboard_text(text.to_owned());
                        driver.delete_selection()
                    }
                }
                "v" => {
                    let text = shell_provider.get_clipboard_text().unwrap_or_default();
                    driver.insert_or_replace_selection(&text)
                }
                _ => unreachable!(),
            }

            return Some(GeneratedEvent::Input);
        }
        Key::Character(c) if action_mod && matches!(c.to_lowercase().as_str(), "a") => {
            if shift {
                driver.collapse_selection()
            } else {
                driver.select_all()
            }
        }
        Key::ArrowLeft => {
            if action_mod {
                if shift {
                    driver.select_word_left()
                } else {
                    driver.move_word_left()
                }
            } else if shift {
                driver.select_left()
            } else {
                driver.move_left()
            }
        }
        Key::ArrowRight => {
            if action_mod {
                if shift {
                    driver.select_word_right()
                } else {
                    driver.move_word_right()
                }
            } else if shift {
                driver.select_right()
            } else {
                driver.move_right()
            }
        }
        Key::ArrowUp => {
            if shift {
                driver.select_up()
            } else {
                driver.move_up()
            }
        }
        Key::ArrowDown => {
            if shift {
                driver.select_down()
            } else {
                driver.move_down()
            }
        }
        Key::Home => {
            if action_mod {
                if shift {
                    driver.select_to_text_start()
                } else {
                    driver.move_to_text_start()
                }
            } else if shift {
                driver.select_to_line_start()
            } else {
                driver.move_to_line_start()
            }
        }
        Key::End => {
            if action_mod {
                if shift {
                    driver.select_to_text_end()
                } else {
                    driver.move_to_text_end()
                }
            } else if shift {
                driver.select_to_line_end()
            } else {
                driver.move_to_line_end()
            }
        }
        Key::Delete => {
            if action_mod {
                driver.delete_word()
            } else {
                driver.delete()
            }
            return Some(GeneratedEvent::Input);
        }
        Key::Backspace => {
            if action_mod {
                driver.backdelete_word()
            } else {
                driver.backdelete()
            }
            return Some(GeneratedEvent::Input);
        }
        Key::Enter => {
            if is_multiline {
                driver.insert_or_replace_selection("\n");
            } else {
                return Some(GeneratedEvent::Submit);
            }
        }
        Key::Character(s) => {
            driver.insert_or_replace_selection(&s);
            return Some(GeneratedEvent::Input);
        }
        _ => {}
    };

    None
}

/// https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#field-that-blocks-implicit-submission
fn implicit_form_submission(doc: &BaseDocument, text_target: usize) {
    let Some(form_owner_id) = doc.controls_to_form.get(&text_target) else {
        return;
    };
    if doc
        .controls_to_form
        .iter()
        .filter(|(_control_id, form_id)| *form_id == form_owner_id)
        .filter_map(|(control_id, _)| doc.nodes[*control_id].element_data())
        .filter(|element_data| {
            element_data.attr(local_name!("type")).is_some_and(|t| {
                matches!(
                    t,
                    "text"
                        | "search"
                        | "email"
                        | "url"
                        | "tel"
                        | "password"
                        | "date"
                        | "month"
                        | "week"
                        | "time"
                        | "datetime-local"
                        | "number"
                )
            })
        })
        .count()
        > 1
    {
        return;
    }

    doc.submit_form(*form_owner_id, *form_owner_id);
}
