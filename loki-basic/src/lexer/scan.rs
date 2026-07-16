// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Whitespace, line-continuation, and comment skipping for the lexer. Split
//! from `lexer/mod.rs` for the 300-line ceiling.

use super::Lexer;

impl Lexer<'_> {
    /// Skips inline whitespace and ` _` line continuations at the cursor.
    pub(super) fn skip_inline_ws_and_continuations(&mut self) {
        loop {
            match self.peek() {
                Some(' ' | '\t') => {
                    self.bump();
                }
                // A `_` followed only by optional inline whitespace and then a
                // line break is a continuation: swallow it and the break so the
                // next token joins this logical line.
                Some('_') if self.is_line_continuation() => {
                    self.consume_line_continuation();
                }
                _ => break,
            }
        }
    }

    fn is_line_continuation(&self) -> bool {
        // Look past the `_` for inline whitespace, then require a line break/EOF.
        let mut j = self.cursor() + 1;
        while let Some(c) = self.char_at(j) {
            match c {
                ' ' | '\t' => j += 1,
                '\n' | '\r' => return true,
                _ => return false,
            }
        }
        true // trailing `_` at EOF
    }

    fn consume_line_continuation(&mut self) {
        self.bump(); // '_'
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.bump();
        }
        match self.peek() {
            Some('\r') => {
                self.bump();
                if self.peek() == Some('\n') {
                    self.bump();
                }
            }
            Some('\n') => {
                self.bump();
            }
            _ => {}
        }
    }

    /// Advances to (but not past) the next line break or EOF.
    pub(super) fn skip_to_line_end(&mut self) {
        while !matches!(self.peek(), Some('\n' | '\r') | None) {
            self.bump();
        }
    }
}
