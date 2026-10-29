//! Stable deterministic replay/mutation smoke, separate from coverage-guided fuzzing.
use paintcraft_container_fuzz::{INPUT_MAX, Outcome, Target, classify, exercise};
use std::{env, fs, io::Read, path::PathBuf};
fn main() {
    let mut args = env::args().skip(1);
    let target = Target::parse(
        &args
            .next()
            .expect("target: avif/heif/paint/pdn/dcx/radiance/exr/raster/tiff_pages/icons"),
    )
    .expect("unknown target");
    let iterations: usize = args
        .next()
        .expect("mutation count per seed")
        .parse()
        .expect("integer count");
    assert!(iterations <= 100_000, "replay iteration allowance");
    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut state = 0x5041_494e_5443_5241u64;
    let mut count = 0u64;
    for path in args.map(PathBuf::from) {
        let mut original = Vec::new();
        fs::File::open(path)
            .expect("open seed")
            .take(INPUT_MAX as u64 + 1)
            .read_to_end(&mut original)
            .expect("read bounded seed");
        assert!(original.len() <= INPUT_MAX, "seed input allowance");
        for iteration in 0..=iterations {
            let mut bytes = original.clone();
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if iteration != 0 {
                match iteration % 4 {
                    0 => bytes.truncate(state as usize % (bytes.len() + 1)),
                    1 if !bytes.is_empty() => {
                        let index = state as usize % bytes.len();
                        bytes[index] ^= (state >> 32) as u8 | 1;
                    }
                    2 if bytes.len() < INPUT_MAX => bytes.push(state as u8),
                    _ if !bytes.is_empty() => {
                        let index = state as usize % bytes.len();
                        bytes[index] = 0xff;
                    }
                    _ => {}
                }
            }
            if classify(exercise(target, &bytes)) == Outcome::Accepted {
                accepted += 1;
            } else {
                rejected += 1;
            }
            count += 1;
        }
    }
    assert!(count > 0, "at least one seed required");
    println!(
        "target={target:?} cases={count} accepted={accepted} rejected={rejected} seed=0x5041494e54435241 mode=deterministic-replay policy=invariants-fail-v1"
    );
}
