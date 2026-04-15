//! Cross-platform GPU utilities on wgpu.
//!
//! Utilities:
//! - `RadixSorter` — 32-bit key-value radix sort (API stub; algorithm lands later).
//! - `TextRenderer` — 3×5 bitmap text overlay for HUDs.

pub mod radix_sort;
pub mod text;

pub use radix_sort::RadixSorter;
pub use text::TextRenderer;
