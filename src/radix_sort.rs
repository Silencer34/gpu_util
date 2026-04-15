//! GPU radix sort for 32-bit key-value pairs.
//!
//! Placeholder — algorithm implementation lands in a follow-up commit.

#![allow(dead_code)]

/// A 32-bit key-value radix sorter.
pub struct RadixSorter {
    _max_n: u32,
}

impl RadixSorter {
    pub fn new(_device: &wgpu::Device, max_n: u32) -> Self {
        Self { _max_n: max_n }
    }

    /// Sort up to `n` key-value pairs in-place.
    ///
    /// Both buffers must have usage `STORAGE | COPY_DST | COPY_SRC` and be
    /// large enough for `n` u32 elements.
    pub fn sort(
        &self,
        _encoder: &mut wgpu::CommandEncoder,
        _keys: &wgpu::Buffer,
        _values: &wgpu::Buffer,
        _n: u32,
    ) {
        // TODO: 4×8-bit LSD radix sort, workgroup-shared Blelloch scan.
        unimplemented!("RadixSorter::sort — landing in a follow-up");
    }
}
