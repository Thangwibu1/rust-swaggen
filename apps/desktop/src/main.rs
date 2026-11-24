#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use eframe::egui;
use std::path::PathBuf;
use swaggen_desktop::app::App;
use swaggen_domain::{Error, LocalId};

fn options() -> Result<(PathBuf, LocalId), Error> {
    let mut root = std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Swaggen"));
    let mut workspace = LocalId::parse("personal")?;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            root = Some(PathBuf::from(args.next().ok_or(Error::Storage)?));
        } else if arg == "--workspace" {
            workspace = LocalId::parse(
                args.next()
                    .ok_or(Error::InvalidId)?
                    .into_string()
                    .map_err(|_| Error::InvalidId)?,
            )?;
        } else {
            return Err(Error::InvalidId);
        }
    }
    let root = root.ok_or(Error::Storage)?;
    if !root.is_absolute() {
        return Err(Error::Storage);
    }
    Ok((root, workspace))
}

fn main() -> eframe::Result {
    let (root, workspace) = match options() {
        Ok(v) => v,
        Err(_) => std::process::exit(2),
    };
    eframe::run_native(
        "Swaggen",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([720.0, 480.0]),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(App::new(cc, root, workspace)))),
    )
}
