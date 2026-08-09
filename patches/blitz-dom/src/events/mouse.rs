use blitz_traits::{
    events::{
        BlitzInputEvent, BlitzMouseButtonEvent, DomEvent, DomEventData, MouseEventButton,
        MouseEventButtons,
    },
    navigation::NavigationOptions,
};
use markup5ever::local_name;

use crate::{BaseDocument, node::SpecialElementData};

pub(crate) fn handle_mousemove(
    doc: &mut BaseDocument,
    target: usize,
    x: f32,
    y: f32,
    buttons: MouseEventButtons,
) -> bool {
    let mut changed = doc.set_hover_to(x, y);

    let Some(hit) = doc.hit(x, y) else {
        return changed;
    };

    if hit.node_id != target {
        return changed;
    }

    let node = &mut doc.nodes[target];
    let Some(el) = node.data.downcast_element_mut() else {
        return changed;
    };

    let disabled = el.attr(local_name!("disabled")).is_some();
    if disabled {
        return changed;
    }

    if let SpecialElementData::TextInput(ref mut text_input_data) = el.special_data {
        if buttons == MouseEventButtons::None {
            return changed;
        }

        let content_box_offset = taffy::Point {
            x: node.final_layout.padding.left + node.final_layout.border.left,
            y: node.final_layout.padding.top + node.final_layout.border.top,
        };

        let x = (hit.x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
        let y = (hit.y - content_box_offset.y) as f64 * doc.viewport.scale_f64();

        text_input_data
            .editor
            .driver(&mut doc.font_ctx.lock().unwrap(), &mut doc.layout_ctx)
            .extend_selection_to_point(x as f32, y as f32);

        changed = true;
    }

    changed
}

pub(crate) fn handle_mousedown(doc: &mut BaseDocument, target: usize, x: f32, y: f32) {
    let Some(hit) = doc.hit(x, y) else {
        return;
    };
    if hit.node_id != target {
        return;
    }

    let node = &mut doc.nodes[target];
    let Some(el) = node.data.downcast_element_mut() else {
        return;
    };

    let disabled = el.attr(local_name!("disabled")).is_some();
    if disabled {
        return;
    }

    if let SpecialElementData::TextInput(ref mut text_input_data) = el.special_data {
        let content_box_offset = taffy::Point {
            x: node.final_layout.padding.left + node.final_layout.border.left,
            y: node.final_layout.padding.top + node.final_layout.border.top,
        };
        let x = (hit.x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
        let y = (hit.y - content_box_offset.y) as f64 * doc.viewport.scale_f64();

        text_input_data
            .editor
            .driver(&mut doc.font_ctx.lock().unwrap(), &mut doc.layout_ctx)
            .move_to_point(x as f32, y as f32);

        doc.set_focus_to(hit.node_id);
        return;
    }

    // PATCH(loki): focus moves on **mousedown**, not on click.
    //
    // It used to happen in `handle_click`, which the driver runs *after* the
    // embedder's own handler — so a handler that mounted an `autofocus` element
    // in response to the click had its focus silently taken back one step later.
    // That is every "click a button, a menu opens" interaction there is: the
    // menu really did receive focus, and then the trigger stole it, leaving a
    // raised overlay whose keys all go to the button behind it.
    //
    // Browsers focus on mousedown for exactly this ordering reason, so this is
    // the platform behaviour rather than a workaround for it. `handle_click`
    // keeps the walk — it still decides where a default action applies — but no
    // longer assigns focus.
    focus_from_pointer(doc, target);
}

/// PATCH(loki): moves focus to the nearest focusable ancestor of a pressed
/// node, or clears it if there is none.
///
/// Clicking a `tabindex="0"` container's child focuses the container, which is
/// what makes subsequent keydowns reach it; clicking inert chrome focuses
/// nothing, which is what makes Tab restart from the top rather than from
/// wherever the user last clicked.
fn focus_from_pointer(doc: &mut BaseDocument, target: usize) {
    let mut maybe_node_id = Some(target);
    while let Some(node_id) = maybe_node_id {
        if doc.nodes[node_id].is_focussable() {
            doc.set_focus_to(node_id);
            return;
        }
        maybe_node_id = doc.nodes[node_id].parent;
    }
    doc.clear_focus();
}

pub(crate) fn handle_mouseup<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    target: usize,
    event: &BlitzMouseButtonEvent,
    mut dispatch_event: F,
) {
    if doc.devtools().highlight_hover {
        let mut node = doc.get_node(target).unwrap();
        if event.button == MouseEventButton::Secondary {
            if let Some(parent_id) = node.layout_parent.get() {
                node = doc.get_node(parent_id).unwrap();
            }
        }
        doc.debug_log_node(node.id);
        doc.devtools_mut().highlight_hover = false;
        return;
    }

    // Determine whether to dispatch a click event
    let do_click = true;
    // let do_click = doc.mouse_down_node.is_some_and(|mouse_down_id| {
    //     // Anonymous node ids are unstable due to tree reconstruction. So we compare the id
    //     // of the first non-anonymous ancestor.
    //     mouse_down_id == target
    //         || doc.non_anon_ancestor_if_anon(mouse_down_id) == doc.non_anon_ancestor_if_anon(target)
    // });

    // Dispatch a click event
    if do_click && event.button == MouseEventButton::Main {
        dispatch_event(DomEvent::new(target, DomEventData::Click(event.clone())));
    }
}

pub(crate) fn handle_click<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    target: usize,
    event: &BlitzMouseButtonEvent,
    mut dispatch_event: F,
) {
    let mut maybe_node_id = Some(target);
    while let Some(node_id) = maybe_node_id {
        // Read is_focussable before borrowing the element data mutably below.
        let is_focussable = doc.nodes[node_id].is_focussable();

        let maybe_element = {
            let node = &mut doc.nodes[node_id];
            node.data.downcast_element_mut()
        };

        let Some(el) = maybe_element else {
            maybe_node_id = doc.nodes[node_id].parent;
            continue;
        };

        let disabled = el.attr(local_name!("disabled")).is_some();
        if disabled {
            return;
        }

        if let SpecialElementData::TextInput(_) = el.special_data {
            return;
        }

        match el.name.local {
            local_name!("input") if el.attr(local_name!("type")) == Some("checkbox") => {
                let is_checked = BaseDocument::toggle_checkbox(el);
                let value = is_checked.to_string();
                dispatch_event(DomEvent::new(
                    node_id,
                    DomEventData::Input(BlitzInputEvent { value }),
                ));
                doc.set_focus_to(node_id);
                return;
            }
            local_name!("input") if el.attr(local_name!("type")) == Some("radio") => {
                let radio_set = el.attr(local_name!("name")).unwrap().to_string();
                BaseDocument::toggle_radio(doc, radio_set, node_id);

                // TODO: make input event conditional on value actually changing
                let value = String::from("true");
                dispatch_event(DomEvent::new(
                    node_id,
                    DomEventData::Input(BlitzInputEvent { value }),
                ));

                BaseDocument::set_focus_to(doc, node_id);

                return;
            }
            // Clicking labels triggers click, and possibly input event, of associated input
            local_name!("label") => {
                if let Some(target_node_id) = doc.label_bound_input_element(node_id).map(|n| n.id) {
                    // Apply default click event action for target node
                    let target_node = doc.get_node_mut(target_node_id).unwrap();
                    let syn_event = target_node.synthetic_click_event_data(event.mods);
                    handle_click(doc, target_node_id, &syn_event, dispatch_event);
                    // PATCH(loki): explicit now that `handle_click` no longer
                    // focuses. A label click focuses its bound input in every
                    // browser, and mousedown hit the *label*, so nothing else
                    // would move focus there.
                    if doc.nodes[target_node_id].is_focussable() {
                        doc.set_focus_to(target_node_id);
                    }
                    return;
                }
            }
            local_name!("a") => {
                if let Some(href) = el.attr(local_name!("href")) {
                    if let Some(url) = doc.url.resolve_relative(href) {
                        doc.navigation_provider.navigate_to(NavigationOptions::new(
                            url,
                            String::from("text/plain"),
                            doc.id(),
                        ));
                    } else {
                        println!("{href} is not parseable as a url. : {:?}", *doc.url)
                    }
                    return;
                } else {
                    println!("Clicked link without href: {:?}", el.attrs());
                }
            }
            local_name!("input")
                if el.is_submit_button() || el.attr(local_name!("type")) == Some("submit") =>
            {
                if let Some(form_owner) = doc.controls_to_form.get(&node_id) {
                    doc.submit_form(*form_owner, node_id);
                }
            }
            #[cfg(feature = "file_input")]
            local_name!("input") if el.attr(local_name!("type")) == Some("file") => {
                use crate::qual_name;
                //TODO: Handle accept attribute https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/accept by passing an appropriate filter
                let multiple = el.attr(local_name!("multiple")).is_some();
                let files = doc.shell_provider.open_file_dialog(multiple, None);

                if let Some(file) = files.first() {
                    el.attrs
                        .set(qual_name!("value", html), &file.to_string_lossy());
                }
                let text_content = match files.len() {
                    0 => "No Files Selected".to_string(),
                    1 => files
                        .first()
                        .unwrap()
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    x => format!("{x} Files Selected"),
                };

                if files.is_empty() {
                    el.special_data = SpecialElementData::None;
                } else {
                    el.special_data = SpecialElementData::FileInput(files.into())
                }
                let child_label_id = doc.nodes[node_id].children[1];
                let child_text_id = doc.nodes[child_label_id].children[0];
                let text_data = doc.nodes[child_text_id]
                    .text_data_mut()
                    .expect("Text data not found");
                text_data.content = text_content;
            }
            _ => {}
        }
        // el borrow released by NLL here.

        // A focusable ancestor ends the walk: its default action, if any, has
        // run, and nothing above it should also act on this click.
        //
        // PATCH(loki): it no longer *assigns* focus — `handle_mousedown` does,
        // before the embedder's handler runs. See `focus_from_pointer`.
        if is_focussable {
            return;
        }

        // No match and not focusable.  Recurse up to parent.
        maybe_node_id = doc.nodes[node_id].parent;
    }
}
