//! Integration tests for RadixSorter.
//!
//! Real tests (randomized, edge cases, key-value correctness) land with
//! the algorithm implementation.

#[test]
fn scaffolding_smoke() {
    // Currently just confirms the crate links. Remove once real tests exist.
    let _ = gpu_util::RadixSorter::new;
}
