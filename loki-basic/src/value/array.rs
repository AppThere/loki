// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! BASIC arrays: multi-dimensional, value-typed storage.

use super::Value;
use crate::error::RuntimeError;

/// Hard cap on total element count, so a hostile `Dim a(2000000000)` cannot
/// request a multi-gigabyte allocation (macro spec §8, memory caps). ~16M
/// elements is far beyond any legitimate macro array.
const MAX_ELEMENTS: usize = 16_000_000;

/// A BASIC array. Value-typed (cloned on assignment), with one `(lower, upper)`
/// bound pair per dimension and a flat, row-major element buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    /// Inclusive `(lower, upper)` bound for each dimension.
    dims: Vec<(i32, i32)>,
    /// Elements in row-major order.
    data: Vec<Value>,
}

impl Array {
    /// Creates an array with the given per-dimension inclusive bounds, filled
    /// with `Empty`. An empty `dims` list yields a zero-dimension (unallocated)
    /// array, as produced by `Dim a()`.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::subscript_out_of_range`] if any bound is inverted
    /// or the total element count overflows.
    pub fn new(dims: Vec<(i32, i32)>) -> Result<Self, RuntimeError> {
        let mut count: usize = 1;
        for &(lo, hi) in &dims {
            if hi < lo - 1 {
                return Err(RuntimeError::subscript_out_of_range());
            }
            let len = (i64::from(hi) - i64::from(lo) + 1).max(0) as usize;
            count = count
                .checked_mul(len)
                .ok_or_else(RuntimeError::subscript_out_of_range)?;
        }
        if dims.is_empty() {
            count = 0;
        }
        if count > MAX_ELEMENTS {
            return Err(RuntimeError::subscript_out_of_range());
        }
        Ok(Self {
            dims,
            data: vec![Value::Empty; count],
        })
    }

    /// Number of dimensions.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// The elements in row-major order (for `For Each`).
    #[must_use]
    pub fn values(&self) -> &[Value] {
        &self.data
    }

    /// The inclusive lower bound of dimension `d` (1-based, VBA `LBound`).
    #[must_use]
    pub fn lbound(&self, d: usize) -> Option<i32> {
        self.dims.get(d.wrapping_sub(1)).map(|&(lo, _)| lo)
    }

    /// The inclusive upper bound of dimension `d` (1-based, VBA `UBound`).
    #[must_use]
    pub fn ubound(&self, d: usize) -> Option<i32> {
        self.dims.get(d.wrapping_sub(1)).map(|&(_, hi)| hi)
    }

    /// Reads the element at `indices` (one per dimension).
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::subscript_out_of_range`] on a bad index count or
    /// an out-of-bounds subscript.
    pub fn get(&self, indices: &[i32]) -> Result<Value, RuntimeError> {
        let flat = self.flat_index(indices)?;
        self.data
            .get(flat)
            .cloned()
            .ok_or_else(RuntimeError::subscript_out_of_range)
    }

    /// Writes `value` at `indices`.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::subscript_out_of_range`] on a bad index count or
    /// an out-of-bounds subscript.
    pub fn set(&mut self, indices: &[i32], value: Value) -> Result<(), RuntimeError> {
        let flat = self.flat_index(indices)?;
        let slot = self
            .data
            .get_mut(flat)
            .ok_or_else(RuntimeError::subscript_out_of_range)?;
        *slot = value;
        Ok(())
    }

    fn flat_index(&self, indices: &[i32]) -> Result<usize, RuntimeError> {
        if indices.len() != self.dims.len() || self.dims.is_empty() {
            return Err(RuntimeError::subscript_out_of_range());
        }
        // Row-major (first index most significant): flat = flat*extent + offset.
        let mut flat: usize = 0;
        for (&idx, &(lo, hi)) in indices.iter().zip(&self.dims) {
            if idx < lo || idx > hi {
                return Err(RuntimeError::subscript_out_of_range());
            }
            let extent = (i64::from(hi) - i64::from(lo) + 1) as usize;
            let offset = (i64::from(idx) - i64::from(lo)) as usize;
            flat = flat * extent + offset;
        }
        Ok(flat)
    }
}
