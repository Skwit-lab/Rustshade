# Hooks: DirectX, Vulkan, OpenGL

Zwei Integrationswege. Beide nutzen ausschließlich den Stable-Compiler
(kein `retour`, keine Nightly-Features) — gepatcht werden COM-VTables.

## 1. Injection (Windows)

`rustshade-hook` baut eine cdylib (`rustshade_hook.dll`), die `DllMain`
exportiert. Der `injector` lädt sie per `LoadLibraryW` +
`CreateRemoteThread` in den Zielprozess.

In `DllMain` (Worker-Thread, kein Loader-Lock) passiert:

1. Preset aus `rustshade.toml` laden (erst neben der DLL, dann im
   Arbeitsverzeichnis des Spiels).
2. **D3D11** (`d3d11.rs`): Dummy-Device + Swap-Chain erzeugen, `Present`
   aus VTable-Slot 8 der Swap-Chain patchen.
3. **D3D9** (`d3d9.rs`): Dummy-Device über `Direct3DCreate9` erzeugen,
   dann `Present` (Slot 17), `Reset` (Slot 16), `CreateDepthStencilSurface`
   (Slot 29) und `SetDepthStencilSurface` (Slot 39) der Device-VTable
   patchen. Es wird nur gepatcht, wenn d3d9.dll/d3d11.dll im Prozess
   geladen sind.
4. Statusmeldungen landen im Debug-Output (`OutputDebugStringW`, mit
   DebugView lesbar, Präfix `rustshade:`).

### Depth Buffer (ReShade-artig, D3D9)

Der `CreateDepthStencilSurface`-Hook ersetzt `D3DFMT_D24S8` still durch
`D3DFMT_INTZ` (FourCC `0x5A544E49`). INTZ ist eine von allen aktuellen
D3D9-GPUs unterstützte vendoren Erweiterung, die es Pixel-Shadern
erlaubt, die Tiefen-Surface wie eine normale Textur zu samplen — der
gleiche Mechanismus, den ReShade für Depth-Effekte nutzt. Der zuletzt
gebundene Depth-Stencil wird getrackt (Set-Depth-Stencil-Hook auf
Slot 39, Reset via `Reset`-Hook auf Slot 16).

### Overlay

Noch nicht implementiert. Geplanter Weg: eigener Quad-Renderer im
D3D9-Hook (FVF-Vertices + compiliertes Pixel-Shader via `D3DCompile`
aus d3dcompiler_47.dll), Bitmap-Font statt D3DXFont. Roadmap:
`docs/ROADMAP.md`.

## 2. Vulkan Layer (Windows + Linux) — geplant

ReShade-artig als impliziter/explicit Vulkan layer (JSON-Manifest in
`~/.local/share/vulkan/explicit_layer.d` bzw. Registry):

- Hook von `vkQueuePresentKHR` / `vkQueueSubmit`, eigener Command-Buffer
  davor einfügen.
- wgpu fährt nativ auf Vulkan — der GpuRenderer kann hier fast direkt
  genutzt werden.

## 3. OpenGL (optional, geplant)

Linux: `glXSwapBuffers` per LD_PRELOAD abfangen, Frame via
`glReadPixels`/FBO-Pass pipen. Windows: `wglSwapLayerBuffers`.
Niedrige Priorität (Vulkan/DX überwiegen).

## Portal 2 (Beispiel: Source Engine)

- **Windows:** Portal 2 rendert über **D3D9** — der D3D9-Hook greift hier
  automatisch (`Present` + Depth-Trace). DLL injizieren mit dem Injector
  (`LoadLibraryW` + `CreateRemoteThread`) oder jedem kompatiblen
  LoadLibrary-Injector.
- **Linux:** Portal 2 nutzt OpenGL → der OpenGL-Weg (Punkt 3) steht noch
  aus.
- Singleplayer bzw. Nicht-VAC-Server verwenden; auf VAC-geschützten
  Servern keine fremde DLL injizieren.

## Anti-Cheat-Hinweis

Injection in Multiplayer-Spielen mit Anti-Cheat (EAC, BattlEye, VAC bei
Online-Spielen) kann geblockt werden und führt zu Bans. rustshade zielt
nur auf legitime Anwendungen (Singleplayer, eigene Engine, Screenshot-/
Render-Pipelines).
