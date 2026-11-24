# Roadmap

## Phase 1 — aktuell (Alpha)
- [x] Preset-Format + Registry + Validierung + Hot-Reload-Loader
- [x] CPU-Referenz-Effekte (9 Effekte, getestet)
- [x] wgpu-GPU-Backend (alle Effekte inkl. multi-pass Bloom + 3D-LUT)
- [x] CLI (apply/validate/init/list) mit CPU-Fallback
- [x] Presets (default, cinematic, competitive)
- [ ] CI: `cargo fmt`/`clippy` + Test-Job (Linux), `cargo check --target x86_64-pc-windows-msvc`-Job (Windows)

## Phase 2 — Live-Anwendung
- [ ] Vulkan-Layer (implizit/explicit, D3D12 + Vulkan-Support über wgpu)
- [ ] D3D11-Hook fertigstellen (auf echter Windows-Hardware verifizieren)
- [ ] Overlay-GUI (dear-imgui-rs) mit Slidern pro Effekt
- [ ] Hot-Reload im Live-Pfad (LoadedPreset::reload_if_changed pro Frame)
- [ ] OpenGL-Weg (LD_PRELOAD/wgl)
- [ ] Screenshot-API + Splitscreen-Vergleichsmodus

## Phase 3 — Erweiterte Effekte
- [ ] CAS-Schärfung (statt unsharp mask) als Alternative
- [ ] SMAA / TAA--artige Kantenglättung
- [ ] Motion-Blur, Depth-of-Field (braucht Depth-Buffer → Phase 2 Hook)
- [ ] Tonemapping-Optionen (ACES, AgX-like)

## SDFGI / Global Illumination

Langfristig - wie in Godot ("SDFGI") oder als Fullbright-Debug - wollen wir teils **illuminations-nahe Pässe** anbieten. Ehrlich eingeordnet:

- Tools, die nur in den finalen Frame injizieren, sehen grundsätzlich nur das fertige 2D-Bild — echtes Feedback-basiertes Lighting (Godot SDFGI sitzt im Engine-Renderer) ist dadurch nur **approximativ** möglich.
- Der machbare Weg über Hooks:
  1. **Depth-Buffer-Extraktion** pro Frame (im Hook verfügbar) → Tiefenbild.
  2. **Frame-Historie** zu einer temporären 3D-Voxel-/TSDF-Approximation der Szene akkumulieren (Kamera-Bewegung nötig; ohne Motion-Vektoren grob).
  3. Aus der SDF/TSDF-Akkumulation diffuse GI-Schätzung (DDGI-artige Tracings, konsistent mit Radiance Cascades wäre ebenfalls eine Option) und als additiver Pass auf den Frame legen.
  4. **Fullbright**-Modus = einfacher Fall: nur Materialfarben ohne Schattierung nachbilden, debug-tauglich sobald Depth/Normals vorliegen.
- Architekturvorarbeit ist drin: `Effect::uses_depth()` / erweiterbares `Ctx`, Offenes Pipeline-Design für Multi-Ressource-Pässe.

Konkret geplant als `rustshade-gi`-Crate mit eigenen Registrierungen (`sdfgi`, `fullbright`, `depth_fog`, `depth_of_field`), das nur aktiv wird, wenn ein Backend Depth-Daten liefern kann (Hook-Pfade), mit Fallback-Verhalten „pass disabled" für standalone CLIs.
