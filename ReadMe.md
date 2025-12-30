# 🎯 Custicle 🎯
(🚧 work in progress 👷🏼)

**Custicle** is a Rust-based crosshair configurator for video games running in **windowed fullscreen mode**. It provides a fully customizable overlay for crosshairs, giving gamers precise control over appearance and behavior.  

Built with Rust, Custicle leverages:

- [`winit`](https://crates.io/crates/winit) – for cross-platform window creation and event handling.  
- [`ash`](https://crates.io/crates/ash) – Vulkan bindings for high-performance rendering.  
- [`ash-window`](https://crates.io/crates/ash-window) – integration between `winit` windows and Vulkan surfaces.  

---

## Planned Features

- Customizable crosshair styles, colors, and sizes.  
- Smooth overlay rendering on games in **windowed fullscreen** mode.  
- Lightweight and high-performance, thanks to Rust + Vulkan.  

---

## Getting Started

### Requirements

- Rust stable toolchain
- Vulkan-compatible GPU and drivers

### Build

Clone the repository and build with Cargo:

```bash
git clone https://github.com/yourusername/custicle.git
cd custicle
cargo build --release
```

### Run
```bash
cargo run --release
```

The overlay window will appear, allowing you to configure your crosshair in real time.

---

## License

Custicle is licensed under the **Creative Commons Attribution-NonCommercial 4.0 International (CC BY-NC 4.0)** license.  
You may use, modify, and share the software **for non-commercial purposes only**. See [LICENSE](https://creativecommons.org/licenses/by-nc/4.0/deed.en) for full details.
