// SPDX-License-Identifier: Apache-2.0

//! The binary WebSocket frame format.
//!
//! One tag byte followed by an opaque payload. The server never parses the
//! payload — under Tier 2 it is ciphertext by construction.

/// Frame tag for a Loro update (persisted to the oplog, ADR-C013).
const TAG_UPDATE: u8 = 0x01;
/// Frame tag for awareness (presence/cursors — broadcast-only, never persisted).
const TAG_AWARENESS: u8 = 0x02;

/// A decoded collaboration frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollabFrame {
    /// An opaque Loro update.
    Update(Vec<u8>),
    /// An opaque awareness payload (cursors/selection).
    Awareness(Vec<u8>),
}

impl CollabFrame {
    /// Encodes the frame for the wire.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let (tag, payload) = match self {
            Self::Update(p) => (TAG_UPDATE, p),
            Self::Awareness(p) => (TAG_AWARENESS, p),
        };
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.extend_from_slice(payload);
        out
    }

    /// Decodes a frame received from the wire.
    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        let (&tag, payload) = bytes.split_first().ok_or(FrameError::Empty)?;
        match tag {
            TAG_UPDATE => Ok(Self::Update(payload.to_vec())),
            TAG_AWARENESS => Ok(Self::Awareness(payload.to_vec())),
            other => Err(FrameError::UnknownTag(other)),
        }
    }

    /// The opaque payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        match self {
            Self::Update(p) | Self::Awareness(p) => p,
        }
    }
}

/// Frame decode failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    /// Zero-length frame.
    #[error("empty frame")]
    Empty,
    /// Unknown tag byte (client newer than server, or corruption).
    #[error("unknown frame tag {0:#04x}")]
    UnknownTag(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        for frame in [
            CollabFrame::Update(b"loro-bytes".to_vec()),
            CollabFrame::Awareness(vec![]),
        ] {
            assert_eq!(CollabFrame::decode(&frame.encode()), Ok(frame));
        }
    }

    #[test]
    fn bad_frames_are_typed() {
        assert_eq!(CollabFrame::decode(&[]), Err(FrameError::Empty));
        assert_eq!(CollabFrame::decode(&[0x7f, 1]), Err(FrameError::UnknownTag(0x7f)));
    }
}
