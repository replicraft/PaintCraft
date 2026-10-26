use jpegxr::{decode_file, transcode_file_to_path, Decoder};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    if let Err(err) = run() {
        eprintln!("jpegxr: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let Some(command) = args.next() else {
        usage();
        std::process::exit(2);
    };

    match command.to_string_lossy().as_ref() {
        "info" => {
            let input = required_path(args.next(), "input")?;
            let mut decoder = Decoder::open(&input)?;
            let (width, height) = decoder.dimensions()?;
            let pixel_format = decoder.pixel_format()?;
            println!("width={width}");
            println!("height={height}");
            println!("bits_per_pixel={}", pixel_format.bits_per_pixel());
            println!("channels={}", pixel_format.channel_count());
        }
        "decode-raw" => {
            let input = required_path(args.next(), "input")?;
            let output = required_path(args.next(), "output")?;
            let image = decode_file(input)?;
            let mut file = File::create(output)?;
            file.write_all(image.pixels())?;
        }
        "transcode" => {
            let input = required_path(args.next(), "input")?;
            let output = required_path(args.next(), "output")?;
            transcode_file_to_path(input, output)?;
        }
        _ => {
            usage();
            std::process::exit(2);
        }
    }

    Ok(())
}

fn required_path(value: Option<std::ffi::OsString>, name: &str) -> Result<PathBuf, String> {
    value
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing {name} path"))
}

fn usage() {
    eprintln!("usage:");
    eprintln!("  jpegxr info <input.jxr>");
    eprintln!("  jpegxr decode-raw <input.jxr> <output.raw>");
    eprintln!("  jpegxr transcode <input.jxr> <output.jxr>");
}
