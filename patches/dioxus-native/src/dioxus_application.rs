use blitz_shell::{BlitzApplication, View};
use dioxus_core::{provide_context, Event, ScopeId};
use dioxus_history::{History, MemoryHistory};
use dioxus_html::PlatformEventData;
use futures_channel::oneshot;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::DioxusNativeWindowRenderer;
use crate::{
    contexts::DioxusNativeDocument, BlitzShellEvent, DioxusDocument, MountedBackend, MountedElement,
    NodeGeometryData, WindowConfig,
};

/// PATCH(loki): [`MountedBackend`] transport that performs mounted-element
/// actions by posting [`DioxusNativeEvent`]s back to the event loop, where the
/// live document can be mutated/read.
struct ProxyMountedBackend {
    proxy: EventLoopProxy<BlitzShellEvent>,
    window: WindowId,
}

impl MountedBackend for ProxyMountedBackend {
    fn scroll_node_to(&self, node_id: usize, x: f64, y: f64) {
        let _ = self.proxy.send_event(BlitzShellEvent::embedder_event(
            DioxusNativeEvent::ScrollNode {
                window: self.window,
                node_id,
                x,
                y,
            },
        ));
    }

    fn query_geometry(
        &self,
        node_id: usize,
    ) -> Pin<Box<dyn Future<Output = Option<NodeGeometryData>>>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.proxy.send_event(BlitzShellEvent::embedder_event(
            DioxusNativeEvent::QueryNodeGeometry {
                window: self.window,
                node_id,
                reply: Arc::new(Mutex::new(Some(tx))),
            },
        ));
        // Resolves to None if the node is gone (sender dropped without a reply).
        Box::pin(async move { rx.await.ok() })
    }
}

/// Dioxus-native specific event type
pub enum DioxusNativeEvent {
    /// A hotreload event, basically telling us to update our templates.
    #[cfg(all(
        feature = "hot-reload",
        debug_assertions,
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    DevserverEvent(dioxus_devtools::DevserverMsg),

    /// Create a new head element from the Link and Title elements
    ///
    /// todo(jon): these should probabkly be synchronous somehow
    CreateHeadElement {
        window: WindowId,
        name: String,
        attributes: Vec<(String, String)>,
        contents: Option<String>,
    },

    /// PATCH(loki): scroll a node to an absolute `(x, y)` offset (CSS px).
    /// Backs `MountedData::scroll`.
    ScrollNode {
        window: WindowId,
        node_id: usize,
        x: f64,
        y: f64,
    },

    /// PATCH(loki): read a node's scroll / client geometry, replying through the
    /// one-shot channel. Backs `MountedData::get_scroll_offset` / `get_scroll_size`
    /// / `get_client_rect`. The sender is wrapped so it can be taken out of a
    /// shared `&` event; if no reply is sent, the receiver cancels and the query
    /// resolves to `None`.
    QueryNodeGeometry {
        window: WindowId,
        node_id: usize,
        reply: Arc<Mutex<Option<oneshot::Sender<NodeGeometryData>>>>,
    },
}

pub struct DioxusNativeApplication {
    pending_window: Option<WindowConfig<DioxusNativeWindowRenderer>>,
    inner: BlitzApplication<DioxusNativeWindowRenderer>,
    proxy: EventLoopProxy<BlitzShellEvent>,
}

impl DioxusNativeApplication {
    pub fn new(
        proxy: EventLoopProxy<BlitzShellEvent>,
        config: WindowConfig<DioxusNativeWindowRenderer>,
    ) -> Self {
        Self {
            pending_window: Some(config),
            inner: BlitzApplication::new(proxy.clone()),
            proxy,
        }
    }

    pub fn add_window(&mut self, window_config: WindowConfig<DioxusNativeWindowRenderer>) {
        self.inner.add_window(window_config);
    }

    fn handle_blitz_shell_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: &DioxusNativeEvent,
    ) {
        match event {
            #[cfg(all(
                feature = "hot-reload",
                debug_assertions,
                not(target_os = "android"),
                not(target_os = "ios")
            ))]
            DioxusNativeEvent::DevserverEvent(event) => match event {
                dioxus_devtools::DevserverMsg::HotReload(hotreload_message) => {
                    for window in self.inner.windows.values_mut() {
                        let doc = window.downcast_doc_mut::<DioxusDocument>();

                        // Apply changes to vdom
                        dioxus_devtools::apply_changes(&doc.vdom, hotreload_message);

                        // Reload changed assets
                        for asset_path in &hotreload_message.assets {
                            if let Some(url) = asset_path.to_str() {
                                doc.reload_resource_by_href(url);
                            }
                        }

                        window.poll();
                    }
                }
                dioxus_devtools::DevserverMsg::Shutdown => event_loop.exit(),
                dioxus_devtools::DevserverMsg::FullReloadStart => {}
                dioxus_devtools::DevserverMsg::FullReloadFailed => {}
                dioxus_devtools::DevserverMsg::FullReloadCommand => {}
                _ => {}
            },

            DioxusNativeEvent::CreateHeadElement {
                name,
                attributes,
                contents,
                window,
            } => {
                log::info!("[LOKI/head] CreateHeadElement: {name}");
                if let Some(window) = self.inner.windows.get_mut(window) {
                    let doc = window.downcast_doc_mut::<DioxusDocument>();
                    doc.create_head_element(name, attributes, contents);
                    window.poll();
                    // PATCH: request_redraw() after CSS is applied so the scene is
                    // repainted with the new styles.  On desktop the OS posts a
                    // RedrawRequested automatically after resumed(); on Android no
                    // such automatic redraw occurs, so the head-element CSS would
                    // otherwise be applied but never rendered.
                    log::info!("[LOKI/head] request_redraw()");
                    window.request_redraw();
                } else {
                    log::info!("[LOKI/head] WARNING: window not found in inner.windows");
                }
            }

            DioxusNativeEvent::ScrollNode {
                window,
                node_id,
                x,
                y,
            } => {
                if let Some(view) = self.inner.windows.get_mut(window) {
                    let mut changes = Vec::new();
                    view.doc
                        .scroll_node_to_collect(*node_id, *x, *y, &mut changes);
                    if !changes.is_empty() {
                        // Dispatch `onscroll` so reactive state (e.g. the custom
                        // scrollbar) tracks the programmatic scroll.
                        view.doc.handle_scroll_changes(&changes);
                    }
                    view.poll();
                    view.request_redraw();
                }
            }

            DioxusNativeEvent::QueryNodeGeometry {
                window,
                node_id,
                reply,
            } => {
                let geometry = self.inner.windows.get_mut(window).and_then(|view| {
                    view.doc.get_node(*node_id).map(|node| {
                        let origin = node.absolute_position(0.0, 0.0);
                        let layout = &node.final_layout;
                        NodeGeometryData {
                            scroll_x: node.scroll_offset.x,
                            scroll_y: node.scroll_offset.y,
                            scroll_width: layout.scroll_width() as f64,
                            scroll_height: layout.scroll_height() as f64,
                            client_x: origin.x as f64,
                            client_y: origin.y as f64,
                            client_width: layout.size.width as f64,
                            client_height: layout.size.height as f64,
                        }
                    })
                });
                if let Some(g) = geometry {
                    if let Ok(mut guard) = reply.lock() {
                        if let Some(tx) = guard.take() {
                            let _ = tx.send(g);
                        }
                    }
                }
                // On miss, the sender is dropped with the event Arc and the
                // awaiting query resolves to None.
            }

            // Suppress unused variable warning
            #[cfg(not(all(
                feature = "hot-reload",
                debug_assertions,
                not(target_os = "android"),
                not(target_os = "ios")
            )))]
            #[allow(unreachable_patterns)]
            _ => {
                let _ = event_loop;
                let _ = event;
            }
        }
    }

    /// PATCH(loki): dispatch the `mounted` event for any elements that have
    /// registered an `onmounted` listener since the last pass.  Done here (not in
    /// dioxus-native-dom) because only the application owns the event-loop proxy
    /// the [`MountedBackend`] transport needs.  Cheap when nothing is pending.
    fn flush_mounted(&mut self) {
        let proxy = self.proxy.clone();
        for view in self.inner.windows.values_mut() {
            let window = view.window_id();
            let doc = view.downcast_doc_mut::<DioxusDocument>();
            let pending = doc.take_pending_mounted();
            if pending.is_empty() {
                continue;
            }
            for (element_id, node_id) in pending {
                let backend: Arc<dyn MountedBackend> = Arc::new(ProxyMountedBackend {
                    proxy: proxy.clone(),
                    window,
                });
                let element = MountedElement::new(backend, node_id);
                let data: Rc<dyn Any> = Rc::new(PlatformEventData::new(Box::new(element)));
                doc.vdom
                    .runtime()
                    .handle_event("mounted", Event::new(data, false), element_id);
            }
            view.poll();
            view.request_redraw();
        }
    }
}

impl ApplicationHandler<BlitzShellEvent> for DioxusNativeApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(feature = "tracing")]
        tracing::debug!("Injecting document provider into all windows");

        log::info!("[LOKI/app] resumed() pending={}", self.pending_window.is_some());

        if let Some(config) = self.pending_window.take() {
            let mut window = View::init(config, event_loop, &self.proxy);
            log::info!("[LOKI/app] View::init() done");
            let renderer = window.renderer.clone();
            let window_id = window.window_id();
            let doc = window.downcast_doc_mut::<DioxusDocument>();

            doc.vdom.in_scope(ScopeId::ROOT, || {
                let shared: Rc<dyn dioxus_document::Document> =
                    Rc::new(DioxusNativeDocument::new(self.proxy.clone(), window_id));
                provide_context(shared);
            });

            // Add history
            let history_provider: Rc<dyn History> = Rc::new(MemoryHistory::default());
            doc.vdom
                .in_scope(ScopeId::ROOT, move || provide_context(history_provider));

            // Add renderer
            doc.vdom
                .in_scope(ScopeId::ROOT, move || provide_context(renderer));

            // Queue rebuild
            log::info!("[LOKI/app] calling initial_build()");
            doc.initial_build();
            log::info!("[LOKI/app] initial_build() done");

            // And then request redraw
            window.request_redraw();

            // todo(jon): we should actually mess with the pending windows instead of passing along the contexts
            self.inner.windows.insert(window_id, window);
        }

        self.inner.resumed(event_loop);
        // Dispatch `mounted` for elements created by initial_build / resume.
        self.flush_mounted();
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.inner.window_event(event_loop, window_id, event);
        // A re-render triggered by this event may have mounted new elements.
        self.flush_mounted();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: BlitzShellEvent) {
        match event {
            BlitzShellEvent::Embedder(event) => {
                if let Some(event) = event.downcast_ref::<DioxusNativeEvent>() {
                    self.handle_blitz_shell_event(event_loop, event);
                }
            }
            event => self.inner.user_event(event_loop, event),
        }
        // Polls above (head elements, scroll, vdom work) may have mounted nodes.
        self.flush_mounted();
    }
}
