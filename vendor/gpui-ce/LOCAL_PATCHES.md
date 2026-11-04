# Local patches

This crate is based on the WGPUI source vendored by Pulsar-Native revision
`0f4ee7961addc0084ebec427542c26773c7ea0ac`.

It includes the slab upload and allocation fixes from WGPUI commit
`958175d22c2c3669903a72b0fcebb1748aff18b5` in `src/platform/cross/renderer.rs`,
`src/platform/cross/slab_gpu.rs`, and `src/scene.rs`. Repeated references to a
retained layer now upload its packed data once, slab allocations are planned
before backing GPU buffers are sized, and invalid uploads are rejected and
scheduled for re-recording instead of overrunning the GPU buffer.

The WGPUI license and original crate files are preserved in this directory.
