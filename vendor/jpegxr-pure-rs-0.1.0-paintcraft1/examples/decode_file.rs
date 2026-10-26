use jpegxr::decode_file;
use std::path::PathBuf;

fn main() -> Result<(), jpegxr::Error> {
    let Some(input) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run --example decode_file -- <input.jxr>");
        return Ok(());
    };

    let image = decode_file(&input)?;

    println!("input={}", input.display());
    println!("width={}", image.width());
    println!("height={}", image.height());
    println!("stride={}", image.stride());
    println!("bits_per_pixel={}", image.pixel_format().bits_per_pixel());
    println!("decoded_bytes={}", image.pixels().len());

    Ok(())
}
