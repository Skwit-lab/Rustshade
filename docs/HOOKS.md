# Hooks: DirektX, Vulkan, OpenGL

Zwei Integrationswege:

## 1. Injection (Windows, D3D11)

`rustshade-hook` baut eine DLL, die von `injector` (LoadLibraryW + CreateRemoteThread) in den Spielprozess geladen wird:

1. DLL erzeugt ein Dummy-Device + Swap-Chain,, um den vtable-Slot von `IDXGISwapChain::Present` (Slot 8) zu finden — so geht das ohne hartes Adresshunting (gleiche Methode wie ReShade).
2. `retour::static_detour` wrappet `Present`.
3. Im Hook wird der Backbuffer durch die rustshade-Pipeline geschickt (D3D11-Variante derselben Uniform-Packs), danach callt der Original-Present.
4. `ResizeBuffers`-Hook (Slot 13) resized die Pipeline-Texturen.

**⚠️ Status:** Codegerüst vorhanden (`d3d11.rs`, `injector.rs`), aber in dieser Sandbox nicht kompilierbar/getestet (kein Windows-Toolchain). Vor dem produktiven Einsatz bitte ausbauen und auf echter Hardware verifizieren.

## 2. Vulkan Layer (Windows + Linux)

ReShade-artig als **impliziter/explicit Vulkan layer** (JSON-Manifest in `~/.local/share/vulkan/explicit_layer.d` bzw. Registry) geplant:

- Hook von `vkQueuePresentKHR` / `vkQueueSubmit`, eigenen Command-Buffer davor einfügen.
- wgpu fährt nativ auf Vulkan — der GpuRenderer kann hier fast direkt genutzt werden.

## 3. OpenGL (optional)

Linux: `glXSwapBuffers` per LD_PRELOAD abfangen, Frame via `glReadPixels`/FBO-Pass pipen.
Windows: `wglSwapLayerBuffers`. Aufwand höher, Nutzen sinkend (Vulkan/DX überwiegen) — deshalb niedrige Priorität.

## Anti-Cheat-Hinweis
Injection in Multiplayer-Spielen mit Anti-Cheat (EAC, BattlEye) wird blockiert bzw. führt zu Bans. rustshade zielt nur auf legitime Anwendungen (Singleplayer, eigene Engine, Screenshot-/Render-Pipelines).
