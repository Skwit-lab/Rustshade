# rustshade

Eine ReShade-artige Post-Processing-Bibliothek in **Rust** für **Windows und Linux** (DirektX und Vulkan als Ziele, OpenGL optional).

> **Status: Alpha.** CPU-Backend komplett + 36 getestete Unit/Integration-Fälle, GPU-Backend (wgpu) implementiert und shader-geprüft, D3D11-Hook als experimentelles Scaffold. Siehe [docs/ROADMAP.md](docs/ROADMAP.md).

## Was ist das?

rustshade liest eine TOML-Preset-Datei (analog zu ReShade.ini) und wendet einen stapelbaren Effekt-Stack auf gerenderte Frames an:

- `fxaa` – Kantenglättung
- `bloom` – weiches Glühen heller Flächen
- `sharpen` – Unsharp-Mask-Schärfung
- `grade` – Exposure/Kontrast/Sättigung/Temperatur/Tint/Gamma/Brightness
- `vignette` – Randabdunklung
- `chromatic_aberration` – radiale Farbverschiebung
- `grain` – deterministisches Filmkorn
- `scanlines` – CRT-Look
- `lut` – 3D-Farbkorrektur aus `.cube`-Dateien

Zwei komplettierte Backends mit identischer Uniform-Semantik:

| Backend | Technologie | Status |
|---|---|---|
| `rustshade-fx` | CPU-Referenz (pure Rust) | ✅ 28 Tests |
| `rustshade-gpu` | wgpu → Vulkan/DX12/Metal/GL | ✅ kompiliert + shader-geprüft |
| `rustshade-hook` | D3D11-Present-Detour + Injector | ⚠️ experimentell, Windows-only |

## CLI benutzen

```sh
# Default-Preset erzeugen
rustshade init -p mein_preset.toml

# Preset anwenden (GPU falls verfügbar, sonst CPU)
rustshade apply -i input.png -o output.png -p mein_preset.toml

# Explizit CPU (deterministisch, egal wo)
rustshade apply -i input.png -o out.png -p cinematic.toml --no-gpu --frame 42

# Preset prüfen / Effekte auflisten
rustshade validate -p presets/competitive.toml
rustshade list
```

## Workspace

```
crates/rustshade-core   Preset-Format (TOML), Registry, Validierung, Hot-Reload-Loader
crates/rustshade-fx     CPU-Referenz-Implementierung aller Effekte + Frame-Buffer
crates/rustshade-gpu    wgpu-Renderer + WGSL-Shader (gleiche Uniform-Packing-Semantik)
crates/rustshade-cli    CLI mit apply/validate/init/list + CPU-Fallback
crates/rustshade-hook   D3D11-Hook + Injector (Windows, experimentell)
presets/                Beispiel-Presets (default, cinematic, competitive)
docs/                   Architektur, Hook-Design und Roadmap (SDFGI etc.)
```

## Warum zwei Backends?

Die CPU-Implementierung ist die **autoritative Referenz**: deterministisch, ohne GPU testbar, Rückfallebene in VMs ohne Render-Device. Die WGSL-Shader spiegeln dieselbe Uniform-Semantik, damit ein Preset auf beiden Wegen dasselbe Bild erzeugt (kleine Rundungsunterschiede sind erlaubt).

## Global Illumination (SDFGI etc.) – geplant

Die Effekt-Pipeline (`rustshade-fx::pipeline::Effect`) ist absichtlich über ein offenes `Ctx` definiert (Breite, Höhe, Frame-Index; Erweiterbarkeit um Tiefenpuffer/Normals), damit später Engine-nahe Pässe wie SDFGI-artige Beleuchtung, depth-fog oder Fullbright-Debug-Modi eingehängt werden können, ohne die Effekt-Signaturen zu brechen.
Siehe [docs/ROADMAP.md](docs/ROADMAP.md#sdfgi--global-illumination).

## Bauen

```sh
cargo build --release
cargo test --workspace
```

Windows-Overlay-DLL (experimentell):

```sh
# auf Windows mit MSVC toolchain
cargo build -p rustshade-hook --release
```

Details zum Injection-Ablauf: [docs/HOOKS.md](docs/HOOKS.md).

## License

MIT
