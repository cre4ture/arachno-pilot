# Native bridge area

Use this directory sparingly.

Good reasons to add C++ here:

- TensorRT engine wrappers that are awkward to call directly from Rust
- lower-level Jetson camera APIs when GStreamer is not enough
- vendor SDK adapters

Bad reasons:

- putting the whole robot runtime in C++ because one library needs it
- exposing large C++ class graphs throughout the Rust codebase

Preferred interop rule:

- keep the public Rust-facing API tiny
- use `cxx` for Rust/C++ bridges
- pass plain buffers, structs, and handles across the boundary
