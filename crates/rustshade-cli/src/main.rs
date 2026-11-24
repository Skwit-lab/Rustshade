use clap::{Parser, Subcommand};
use image::RgbaImage;
use rustshade_core::Preset;
use rustshade_fx as fx;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "rustshade",
    version,
    about = "Cross-platform post-processing (ReShade-style) written in Rust"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply a preset to an image file.
    Apply {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long)]
        preset: PathBuf,
        /// Force the CPU reference implementation.
        #[arg(long)]
        no_gpu: bool,
        /// Frame index (affects animated effects like grain).
        #[arg(long, default_value_t = 0)]
        frame: u64,
    },
    /// Validate a preset against the effect registry.
    Validate {
        #[arg(short, long)]
        preset: PathBuf,
    },
    /// Write the built-in default preset to a file.
    Init {
        #[arg(short, long, default_value = "rustshade.toml")]
        preset: PathBuf,
    },
    /// List all effects and their parameters.
    List,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Apply {
            input,
            output,
            preset,
            no_gpu,
            frame,
        } => apply(&input, &output, &preset, no_gpu, frame),
        Command::Validate { preset } => {
            let loaded = preset_data(&preset)?;
            let base = preset.parent().unwrap_or(Path::new("."));
            let _stack = fx::build_pipeline(&loaded, base).map_err(|e| e.to_string())?;
            println!(
                "{}: OK — {} effect slots validated.",
                preset.display(),
                loaded.effects.len()
            );
            Ok(())
        }
        Command::Init { preset } => {
            std::fs::write(
                &preset,
                include_str!("../../rustshade-core/assets/default_preset_inline.toml"),
            )
            .map_err(|e| e.to_string())?;
            println!("Wrote default preset to {}", preset.display());
            Ok(())
        }
        Command::List => {
            for def in rustshade_core::REGISTRY {
                println!("{} — {}", def.id, def.doc);
                for p in def.params {
                    match p.kind {
                        rustshade_core::ParamKind::F32 => println!(
                            "    {:22} default {:.3}, range [{:.3}, {:.3}] — {}",
                            p.name, p.default, p.min, p.max, p.doc
                        ),
                        rustshade_core::ParamKind::Bool => {
                            println!("    {:22} boolean — {}", p.name, p.doc)
                        }
                        rustshade_core::ParamKind::String => {
                            println!("    {:22} string — {}", p.name, p.doc)
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

fn preset_data(path: &Path) -> Result<Preset, String> {
    Preset::load(path).map_err(|e| e.to_string())
}

fn load_image(path: &Path) -> Result<RgbaImage, String> {
    image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())
        .map(|d| d.to_rgba8())
}

fn apply(
    input: &Path,
    output: &Path,
    preset_path: &Path,
    no_gpu: bool,
    frame_index: u64,
) -> Result<(), String> {
    let preset = preset_data(preset_path)?;
    let base = preset_path.parent().unwrap_or(Path::new("."));
    let mut stack = fx::build_pipeline(&preset, base).map_err(|e| e.to_string())?;
    let img = load_image(input)?;

    if no_gpu {
        eprintln!("using CPU reference implementation");
        let out = fx::apply_stack(&mut stack, &img, frame_index);
        out.save(output).map_err(|e| e.to_string())?;
    } else {
        match rustshade_gpu::GpuRenderer::new() {
            Ok(mut renderer) => {
                renderer.set_lut_base_dir(base);
                let out = renderer
                    .apply(&preset.effects, &img, frame_index)
                    .map_err(|e| e.to_string())?;
                out.save(output).map_err(|e| e.to_string())?;
            }
            Err(e) => {
                eprintln!("GPU unavailable ({e}); falling back to CPU implementation");
                let out = fx::apply_stack(&mut stack, &img, frame_index);
                out.save(output).map_err(|e| e.to_string())?;
            }
        }
    }
    println!("wrote {}", output.display());
    Ok(())
}
