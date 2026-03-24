mod mermaid;
mod types;

use std::fs;
use std::path::PathBuf;

use clap::Parser;
use types::ExcalidrawFile;

/// A cli that converts an Excalidraw export into promptable Markdown
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the excalidraw file
    #[arg(short, long)]
    path: PathBuf,
}

fn main() {
    let args = Args::parse();
    let contents = match fs::read_to_string(&args.path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", args.path.display(), e);
            std::process::exit(1);
        }
    };
    let file: ExcalidrawFile = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing JSON: {}", e);
            std::process::exit(1);
        }
    };

    let elements: Vec<_> = file.elements.iter().filter(|e| !e.is_deleted).collect();
    let output = mermaid::generate_mermaid(&elements);
    print!("{}", output);
}
