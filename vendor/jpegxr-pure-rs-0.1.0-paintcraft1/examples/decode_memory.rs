use jpegxr::Decoder;
use std::path::PathBuf;

fn main() -> Result<(), jpegxr::Error> {
    let Some(input) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: cargo run --example decode_memory -- <input.jxr>");
        return Ok(());
    };

    let bytes = std::fs::read(&input)?;
    let mut decoder = Decoder::from_slice(&bytes)?;
    let image = decoder.decode()?;

    println!("input={}", input.display());
    println!("encoded_bytes={}", bytes.len());
    println!("width={}", image.width());
    println!("height={}", image.height());
    println!("decoded_bytes={}", image.pixels().len());

    Ok(())
}
