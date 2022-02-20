with-env {
        "WGPU_ADAPTER_NAME": "780",
        "WGPU_BACKEND": "vulkan",
        "RUST_LOG": "raytracer_driver=debug"
    } {
        pueue add -- cargo run --release -p raytracer-driver -- ./target/debug/libraytracer_render.so
        pueue add -- cargo watch -- cargo build -p raytracer-render
    }
