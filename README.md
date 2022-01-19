# ray-tracer-webgl

Rust/WASM + WebGL2 ray tracer based off of Peter Shirley's *Ray Tracing in One Weekend* series. I initially started this project as a software ray tracing running on pure Rust/WASM, but the render times were so frustratingly slow that I quickly looked into implementing a hardware ray tracer that would take full advantage of the GPU's parallelization power. Once I switched to WebGL, render times went from around 1-6 minutes for a decent render to less than a second, and I was able to implement some realtime ray tracing elements like moving spheres, etc. by averaging low-sample frames together.

Since I'm not doing a ton of matrix manipulations, using Rust here isn't completely necessary. However, one benefit of using Rust/WASM as the WebGL wrapper is that Rust code is highly customizable, making it possible to create GLSL-like structs like `vec3`s that respond to arithmetic operators in the same you'd expect a GLSL shader program to. This means setting up state and passing `vec3` data to the GPU becomes a breeze.