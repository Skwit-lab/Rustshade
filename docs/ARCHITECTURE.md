# Architektur

```
preset.toml
    │
    ▼
rustshade-core  ── Preset (TOML) → gegen die Registry validiert
    │                    │
    │                    └─► EffectSpec { id, enabled, params }
    ▼
build_pipeline(preset)  ──►  Vec<Box<dyn Effect>>     (CPU)                │ Uniform-Packing
                        ──►  GpuRenderer::apply()      (GPU, wgpu)          │ identische Semantik
    │                    ──►  hooked Present()          (D3D11, später Vulkan/GL)
    ▼
output frame
```

## Schichten

1. **rustshade-core** – Effect-Registry als Quelle der Wahrheit: alle IDs, Parameternamen, Defaults, Wertebereiche. Backend-übergreifende Validierung (`validate()`), Preset laden (auch als mtime-basierter Hot-Reload via `LoadedPreset`).
2. **rustshade-fx** – CPU-Referenz: `Frame` (f32 RGBA), jeder Effekt implementiert `Effect { id, apply(&mut Frame, &Ctx) }`. Pipeline = geordnete Effektliste. `Ctx` ist offen und erweiterbar (siehe SDFGI-Roadmap).
3. **rustshade-gpu** – wgpu: 1 Fullscreen-Pass pro Effekt (Ping-Pong-Texturen), Bloom als 4-Pass-Chain (threshold → blur H → blur V → composite), LUT als 3D-Textur. Uniform = 64 Byte: `p0/p1/p2 (12 f32) + dims + frame + pad`.
4. **rustshade-cli** – apply/validate/init/list; nimmt GPU wenn verfügbar, sonst CPU-Fallback.
5. **rustshade-hook** – Windows-only D3D11-DLL (Present-Detour) + injector.exe. Der Hook soll langfristig den hinteren Frame-Backbuffer + Depth-Buffer an GpuRenderer bzw. eine native D3D11-Variante derselben Pipeline weiterreichen.

## Preset-Format

```toml
settings = { linear_workspace = false }

[[effects]]
id = "bloom"
strength = 0.5
```

- Effekte werden **in Array-Reihenfolge** angewendet (wie ReShade-Techniken von oben nach unten).
- Unbekannte IDs/Parameter verhindern das Laden (Validierung statt stiller Ignoranz).
- Werte werden mit Registry-Defaults ergänzt; Bereiche werden erzwungen.

## Extension-Punkte

- `Effect::uses_depth()` – Kennzeichnet, dass ein Effekt den Depth-Buffer braucht (noch kein Backend befüllt ihn; D3D11-Hook kann das später).
- `Ctx.frame_index` – für animierte Effekte (grain, zukünftige TAA/Denoiser-Pässe).
- Registry erweitern = neuen `EffectDef` + CPU-Fx + WGSL-Shader ergänzen; Presets bleiben abwärtskompatibel.
