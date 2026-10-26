use jpegxr::transcode_reader_to_writer;
use std::fs::File;
use std::path::PathBuf;

fn default_output() -> PathBuf {
    std::env::temp_dir().join("jpegxr-pure-rs-example-transcode.jxr")
}

fn main() -> Result<(), jpegxr::Error> {
    let Some(input) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run --example transcode_reader_writer -- <input.jxr> [output.jxr]");
        return Ok(());
    };
    let output = std::env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_output);

    let reader = File::open(&input)?;
    let writer = File::create(&output)?;
    transcode_reader_to_writer(reader, writer)?;

    println!("input={}", input.display());
    println!("output={}", output.display());

    Ok(())
}
