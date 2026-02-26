mod human_format;
mod writer;

use crate::human_format::{format_cook_time, format_size};
use crate::writer::StreamingAssetFileWriter;
use clap::{ArgAction, Parser, Subcommand, ValueHint};
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};
use std::process::exit;
use syrillian_asset::store::streaming::asset_store::StreamingAssetFile;

#[derive(Debug, Parser)]
#[command(
    name = "sypack",
    version,
    about = "Generates a .sya asset package from a folder of source assets"
)]
struct Cli {
    #[arg(short = 'v', long, global = true, action = ArgAction::SetTrue)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Package {
        #[arg(short, long, value_name = "FOLDER", value_hint = ValueHint::DirPath)]
        input: PathBuf,
        #[arg(short, long, value_name = "OUTPUT", value_hint = ValueHint::FilePath)]
        output: PathBuf,
    },
    Ls {
        #[arg(value_name = "PACKAGE", value_hint = ValueHint::FilePath)]
        package: PathBuf,
    },
    View {
        #[arg(value_name = "PACKAGE", value_hint = ValueHint::FilePath)]
        package: PathBuf,
        #[arg(value_name = "ASSET_PATH")]
        asset_path: String,
    },
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Command::Package { input, output } => package_command(input, output, args.verbose),
        Command::Ls { package } => ls_command(package),
        Command::View {
            package,
            asset_path,
        } => view_command(package, asset_path),
    }
}

fn package_command(input: PathBuf, output: PathBuf, verbose: bool) {
    if !input.is_dir() {
        eprintln!("Input path is not a directory: {}", input.display());
        exit(2);
    }

    let output_path = with_extension(&output);

    let result = if verbose {
        StreamingAssetFile::pack_folder_with_progress(&input, &output, |asset_type, path, cook| {
            println!(
                "Packaging {:<18} {:>10} {path}",
                asset_type.name(),
                format_cook_time(cook)
            );
        })
    } else {
        StreamingAssetFile::pack_folder(&input, &output)
    };

    if let Err(err) = result {
        eprintln!("Failed to generate package: {err}");
        exit(1);
    }

    println!("Generated package: {}", output_path.display());
}

fn ls_command(package: PathBuf) {
    let package_path = with_extension(&package);
    let package_file = match StreamingAssetFile::load(&package_path) {
        Ok(package_file) => package_file,
        Err(err) => {
            eprintln!("Failed to read package '{}': {err}", package_path.display());
            exit(1);
        }
    };

    let entries = package_file.entries();
    println!("Package: {}", package_path.display());
    println!("Version: {}", package_file.version());
    println!("Assets: {}", package_file.asset_count());
    println!("Blobs: {}", package_file.blob_count());
    println!(
        "{:<6} {:<18} {:>12} {:>12} {:>6} {:>10} {:>18} Path",
        "Index", "Type", "Meta", "Blob", "Count", "Offset", "Hash"
    );

    let mut total_meta = 0;
    let mut total_blob = 0;

    for (index, entry) in entries.iter().enumerate() {
        let relative_path = entry.relative_path.as_deref().unwrap_or("<unavailable>");
        let size = format_size(entry.size);
        let blob_size = format_size(entry.blob_size);
        total_meta += entry.size;
        total_blob += entry.blob_size;
        println!(
            "{:<6} {:<18} {:>12} {:>12} {:>6} {:>10} 0x{:016x} {}",
            index,
            entry.asset_type.name(),
            size,
            blob_size,
            entry.blob_count,
            entry.offset,
            entry.hash,
            relative_path
        );
    }

    println!(
        "Totals: metadata={}, blobs={}, combined={}",
        format_size(total_meta),
        format_size(total_blob),
        format_size(total_meta + total_blob)
    );
}

fn view_command(package: PathBuf, asset_path: String) {
    let package_path = with_extension(&package);
    let mut package_file = match StreamingAssetFile::load(&package_path) {
        Ok(package_file) => package_file,
        Err(err) => {
            eprintln!("Failed to read package '{}': {err}", package_path.display());
            exit(1);
        }
    };

    let Some(entry) = package_file.entry_by_path(&asset_path) else {
        eprintln!(
            "Asset path not found in package '{}': {}",
            package_path.display(),
            asset_path
        );
        eprintln!(
            "Run `syrillian_asset_packer ls {}` to inspect available paths.",
            package_path.display()
        );
        exit(1);
    };

    let payload = match package_file.read_payload_bytes(&entry) {
        Ok(payload) => payload,
        Err(err) => {
            eprintln!(
                "Failed to read payload for '{}': {err}",
                entry.relative_path.as_deref().unwrap_or("<unavailable>")
            );
            exit(1);
        }
    };

    let json = match serde_json::from_slice::<JsonValue>(&payload) {
        Ok(json) => json,
        Err(err) => {
            eprintln!(
                "Payload for '{}' is not valid JSON: {err}",
                entry.relative_path.as_deref().unwrap_or("<unavailable>")
            );
            exit(1);
        }
    };

    let blobs = package_file.blobs_for_hash(entry.hash);

    println!("Package: {}", package_path.display());
    println!(
        "Path: {}",
        entry.relative_path.as_deref().unwrap_or("<unavailable>")
    );
    println!("Type: {}", entry.asset_type.name());
    println!("Hash: 0x{:016x}", entry.hash);
    println!("Meta Size: {}", format_size(entry.size));
    println!("Blob Total: {}", format_size(entry.blob_size));

    if blobs.is_empty() {
        println!("Blobs: none");
    } else {
        println!("Blobs:");
        for blob in blobs {
            println!(
                "  - {:<28} {:>12}  (elements: {})",
                blob.kind.name(),
                format_size(blob.size),
                blob.element_count
            );
        }
    }

    println!("Metadata JSON:");
    print_json_tree(&json);
}

fn with_extension(path: &Path) -> PathBuf {
    let mut path = path.to_path_buf();
    path.set_extension("sya");
    path
}

fn print_json_tree(value: &JsonValue) {
    match value {
        JsonValue::Object(map) => {
            println!("root");
            let len = map.len();
            for (index, (key, child)) in map.iter().enumerate() {
                print_json_tree_node(key, child, "", index + 1 == len);
            }
        }
        JsonValue::Array(array) => {
            println!("root [{}]", array.len());
            for (index, child) in array.iter().enumerate() {
                let label = format!("[{index}]");
                print_json_tree_node(&label, child, "", index + 1 == array.len());
            }
        }
        _ => println!("root: {}", scalar_to_string(value)),
    }
}

fn print_json_tree_node(label: &str, value: &JsonValue, prefix: &str, is_last: bool) {
    let branch = if is_last { "└──" } else { "├──" };
    let child_prefix = if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}│   ")
    };

    match value {
        JsonValue::Object(map) => {
            println!("{prefix}{branch} {label}");
            let len = map.len();
            for (index, (key, child)) in map.iter().enumerate() {
                print_json_tree_node(key, child, &child_prefix, index + 1 == len);
            }
        }
        JsonValue::Array(array) => {
            println!("{prefix}{branch} {label} [{}]", array.len());
            for (index, child) in array.iter().enumerate() {
                let child_label = format!("[{index}]");
                print_json_tree_node(&child_label, child, &child_prefix, index + 1 == array.len());
            }
        }
        _ => {
            println!("{prefix}{branch} {label}: {}", scalar_to_string(value));
        }
    }
}

fn scalar_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::String(text) => format!("{text:?}"),
        _ => value.to_string(),
    }
}
