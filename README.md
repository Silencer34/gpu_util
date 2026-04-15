# gpu_util

Cross-platform GPU utilities built on [wgpu](https://wgpu.rs/).

Runs anywhere wgpu runs: Vulkan, Metal, DirectX 12, and WebGPU (browser).
No CUDA, no vendor SDKs, no subgroup hacks.

## Status

**Scaffolding only.** API surface is stable; the radix-sort algorithm
lands in a follow-up commit. Nothing here is usable in production yet.

## Utilities

- [ ] `RadixSorter` — 32-bit key-value radix sort, LSD, 4×8-bit passes (in progress).
- Future: prefix-scan, segmented scan, reduce, bitonic for in-workgroup sorts.

## Planned API

```rust
use gpu_util::RadixSorter;

let sorter = RadixSorter::new(&device, max_n);
let mut encoder = device.create_command_encoder(&Default::default());
sorter.sort(&mut encoder, &keys, &values, n);
queue.submit(std::iter::once(encoder.finish()));
```

Calling `sort` today will panic — the algorithm isn't wired yet.

## Design notes

- **No subgroup ops.** Baseline WebGPU doesn't expose them, and we don't
  want to guess subgroup sizes. Within-workgroup reductions use shared
  memory.
- **Deterministic.** Given the same input, the same output comes back,
  even across hardware with different SIMD widths.
- **Key-value sort.** Keys are `u32` (unsigned). Values are also `u32`
  (typically used as indices into a parallel array).

## License

Dual-licensed under either of

 - Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
   <http://www.apache.org/licenses/LICENSE-2.0>)
 - MIT license ([LICENSE-MIT](LICENSE-MIT) or
   <http://opensource.org/licenses/MIT>)

at your option.
