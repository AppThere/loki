//! PATCH(loki): `onmounted` / [`MountedData`] support for dioxus-native.
//!
//! Upstream dioxus-native-dom leaves `convert_mounted_data` as
//! `unimplemented!()`, so `onmounted` never fires and `MountedData` queries
//! (`scroll`, `get_scroll_offset`, …) are unavailable.  This module supplies the
//! [`RenderedElementBacking`] implementation that backs those queries.
//!
//! The live document lives on the winit event-loop side, so the backing cannot
//! touch it synchronously.  Instead it delegates to a [`MountedBackend`]
//! transport — implemented in `dioxus-native`, which owns the event-loop proxy —
//! keeping this crate free of any winit / shell dependency.

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use dioxus_html::geometry::euclid::Point2D;
use dioxus_html::geometry::{Pixels, PixelsRect, PixelsSize, PixelsVector2D};
use dioxus_html::{
    MountedError, MountedResult, RenderedElementBacking, ScrollBehavior, ScrollToOptions,
};

use crate::NodeId;

/// Geometry snapshot of a scrollable element, in CSS pixels.  `scroll_width` /
/// `scroll_height` are the *scrollable distance* (content − client), matching
/// the DOM `scroll` event geometry produced elsewhere in this crate.
#[derive(Clone, Copy, Debug, Default)]
pub struct NodeGeometryData {
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub scroll_width: f64,
    pub scroll_height: f64,
    pub client_x: f64,
    pub client_y: f64,
    pub client_width: f64,
    pub client_height: f64,
}

/// Transport that performs mounted-element actions against the live document on
/// the event-loop side.  Implemented in `dioxus-native`.
pub trait MountedBackend: Send + Sync {
    /// Scroll node `node_id` to an absolute `(x, y)` offset (CSS px).
    fn scroll_node_to(&self, node_id: NodeId, x: f64, y: f64);

    /// Asynchronously read `node_id`'s scroll / client geometry.  Resolves to
    /// `None` if the node is gone or the query could not be answered.
    fn query_geometry(
        &self,
        node_id: NodeId,
    ) -> Pin<Box<dyn Future<Output = Option<NodeGeometryData>>>>;
}

/// [`RenderedElementBacking`] for one mounted blitz node, pairing its id with a
/// [`MountedBackend`] transport.
#[derive(Clone)]
pub struct MountedElement {
    backend: Arc<dyn MountedBackend>,
    node_id: NodeId,
}

impl MountedElement {
    pub fn new(backend: Arc<dyn MountedBackend>, node_id: NodeId) -> Self {
        Self { backend, node_id }
    }
}

impl RenderedElementBacking for MountedElement {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn get_scroll_offset(&self) -> Pin<Box<dyn Future<Output = MountedResult<PixelsVector2D>>>> {
        let fut = self.backend.query_geometry(self.node_id);
        Box::pin(async move {
            fut.await
                .map(|g| PixelsVector2D::new(g.scroll_x, g.scroll_y))
                .ok_or(MountedError::NotSupported)
        })
    }

    fn get_scroll_size(&self) -> Pin<Box<dyn Future<Output = MountedResult<PixelsSize>>>> {
        let fut = self.backend.query_geometry(self.node_id);
        Box::pin(async move {
            fut.await
                .map(|g| {
                    PixelsSize::new(
                        g.client_width + g.scroll_width,
                        g.client_height + g.scroll_height,
                    )
                })
                .ok_or(MountedError::NotSupported)
        })
    }

    fn get_client_rect(&self) -> Pin<Box<dyn Future<Output = MountedResult<PixelsRect>>>> {
        let fut = self.backend.query_geometry(self.node_id);
        Box::pin(async move {
            fut.await
                .map(|g| {
                    PixelsRect::new(
                        Point2D::<f64, Pixels>::new(g.client_x, g.client_y),
                        PixelsSize::new(g.client_width, g.client_height),
                    )
                })
                .ok_or(MountedError::NotSupported)
        })
    }

    fn scroll(
        &self,
        coordinates: PixelsVector2D,
        _behavior: ScrollBehavior,
    ) -> Pin<Box<dyn Future<Output = MountedResult<()>>>> {
        self.backend
            .scroll_node_to(self.node_id, coordinates.x, coordinates.y);
        Box::pin(async { Ok(()) })
    }

    fn scroll_to(
        &self,
        _options: ScrollToOptions,
    ) -> Pin<Box<dyn Future<Output = MountedResult<()>>>> {
        // scrollIntoView needs the scrollable ancestor's geometry, which this
        // node-local backing does not have. Callers that need precise control
        // (draggable scrollbar, scroll-to-cursor) use `scroll` with explicit
        // coordinates instead, so this stays NotSupported rather than guessing.
        Box::pin(async { Err(MountedError::NotSupported) })
    }
}
