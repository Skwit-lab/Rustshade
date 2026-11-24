// Windows-only hook implementation lives behind cfg; the crate also builds
// as a stub on Linux so the workspace stays green on all platforms.

#[cfg(windows)]
pub mod d3d11;

#[cfg(windows)]
pub mod injector;
