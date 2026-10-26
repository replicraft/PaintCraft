# jpegxr-pure-rs

Faithful Rust translation of the JXRLib JPEG-XR (based on JXRLib "JPEG XR Device Porting Kit v1.0 April 2013)

* 2026-07-15: Basic testing done




## ⚠ This is an LLM-mediated faithful (hopefully) translation, not the original code!

Most users should probably first see if the existing original code works for them, unless they have
reason otherwise. The original source may have newer features and it has had more love in terms of
fixing bugs. In fact, we aim to replicate (logical) bugs if they are present, for the sake of reproducibility!
(but then we might have added a few more bugs in the process)

There are however cases when you might prefer this Rust version. We generally agree with
[this manifesto](https://rewrites.bio/) but more specifically:

* We have had many issues with ensuring that our software works using existing containers (Docker,
  PodMan, Singularity). One size does not fit all and it eats our resources trying to keep up with
  every way of delivering software
* Common package managers do not work well. It was great when we had a few Linux distributions with
  stable procedures, but now there are just too many ecosystems (Homebrew, Conda). Conda has an
  NP-complete resolver which does not scale. Homebrew is only so-stable. And our dependencies in
  Python still break. These can no longer be considered professional serious options. Meanwhile, Cargo
  enables multiple versions of packages to be available, even within the same program(!)
* The future is the web. We deploy software in the web browser, and until now that has meant
  Javascript. This is a language where even the == operator is broken. Typescript is one step up, but
  a game changer is the ability to compile Rust code into webassembly, enabling performance and
  sharing of code with the backend. Translating code to Rust enables new ways of deployment and
  running code in the browser has especial benefits for science - researchers do not have deep pockets
  to run servers, so pushing compute to the user enables deployment that otherwise would be impossible
* Old CLI-based utilities are bad for the environment(!). A large amount of compute resources are
  spent creating and communicating via small files, which we can bypass by using code as libraries.
  Even better, we can avoid frequent reloading of databases by hoisting this stage, with up to 100x
  speedups in some cases. Less compute means faster compute and less electricity wasted
* LLM-mediated translations may actually be safer to use than the original code. This article shows
  that
  [running the same code on different operating systems can give somewhat different answers](https://doi.org/10.1038/nbt.3820).
  This is a gap that Rust+Cargo can reduce. Typesafe interfaces also reduce coding mistakes and error
  handling, as opposed to typical command-line scripting

But:

* **LLM-mediated translation should still be considered experimental**. The LLM technology is immature and has
  sharp corners. But there are opportunities to reap, and the genie is not going back into the bottle.
  This translation is as much aimed to learn how to improve the translation technology and get feedback
  on the results.
* Translations are not endorsed by the original authors unless otherwise noted. **Do not send bug
  reports to the original developers**. Use our Github issues page instead.
* **Do not trust the benchmarks on this page**. They are used to audit the translation and not aimed to be precise
* **If you want improved performance, you should use this crate as a library, not as a CLI utility**.
  By calling the code directly you avoid startup cost and the need to read/write the results. You also
  get type-safety, and it will be easier to have multiple versions installed in parallel.
* **Check the original Github pages for information about the package**. This README is kept sparse on
  purpose. It is not meant to be the primary source of information.
* **If you are the author of the original code and wish to move to Rust, you can obtain ownership of
  this repository and crate**. Until then, our commitment is to offer an as-faithful-as-possible
  translation of a snapshot of your code. If we find serious bugs, we will report them to you.
  Otherwise we will just replicate them, to ensure comparability across studies that claim to use
  package XYZ v.666. Think of this like a fancy Ubuntu .deb-package of your software - that is how we
  treat it

This blurb might be out of date. Go to
[this page](https://github.com/henriksson-lab/rustification) for the latest information and further
information about how we approach translation.





## Public API

The crate root exposes the intended safe API. The translated JXRLib modules are
internal implementation detail.

Basic use:

```rust
fn main() -> Result<(), jpegxr::Error> {
    let bytes = std::fs::read("image.jxr")?;
    let image = jpegxr::Decoder::from_slice(&bytes)?.decode()?;

    println!(
        "{}x{}, {} bits/pixel",
        image.width(),
        image.height(),
        image.pixel_format().bits_per_pixel()
    );

    std::fs::write("image.raw", image.pixels())?;
    Ok(())
}
```

Decode from a file:

```rust
let image = jpegxr::decode_file("image.jxr")?;
println!("{}x{}", image.width(), image.height());
let pixels = image.pixels();
```

Decode data that is already in memory:

```rust
let bytes = std::fs::read("image.jxr")?;
let image = jpegxr::Decoder::from_slice(&bytes)?.decode()?;
```

Decode from any `Read` source:

```rust
let image = jpegxr::decode_reader(reader)?;
```

Encode or transcode to any `Write` target:

```rust
jpegxr::encode_to_writer(&image, writer)?;
jpegxr::transcode_reader_to_writer(reader, writer)?;
```

## Command line interface

The optional command-line binary provides simple inspection and conversion
helpers. Enable it with the default-off `cli` feature:

```sh
cargo run --features cli --bin jpegxr -- info input.jxr
cargo run --features cli --bin jpegxr -- decode-raw input.jxr output.raw
cargo run --features cli --bin jpegxr -- transcode input.jxr output.jxr
```

Runnable examples:

```sh
cargo run --example decode_file -- image.jxr
cargo run --example decode_memory -- image.jxr
cargo run --example transcode_reader_writer -- input.jxr output.jxr
```



## Real-Data Validation

In the full development repository, pinned JPEG-XR fixtures are recorded under
`fixtures/` and can be refreshed with:

```sh
bash tools/download-fixtures.sh small
bash tools/download-fixtures.sh first-tiles
python3 tools/extract-czi-jpegxr-fixtures.py \
  path/to/Zeiss-5-Cropped.czi \
  fixtures/data/czi-openslide-zeiss5-cropped-small \
  --count 10 \
  --prefix Zeiss-5-Cropped
```

The integration tests in `tests/real_fixtures.rs` open the pinned fixtures
through the translated decoder and compare decoded bytes against the local
upstream `jpegxr` `0.3.1` JXRLib wrapper. This oracle is a dev-dependency only.
The same integration test also runs translated transcode on `sample.jxr`,
decodes the translated output with upstream JXRLib, and compares pixels against
the source. Direct encode is covered by encoding decoded `red.jxr` pixels with
the translated encoder and checking that upstream JXRLib can decode the result
with the expected dimensions and bit depth.



### Decode Benchmark

The full development repository includes matching release benchmark examples
for the upstream JXRLib wrapper and this translated decoder. Reproduce the
parity check and benchmark from the repository root with:

```sh
CARGO_TARGET_DIR=.tmp/target cargo test --test real_fixtures
CARGO_TARGET_DIR=.tmp/target cargo build --release --examples

mkdir -p .tmp/bench
for decoder in original translated; do
  for run in 1 2 3; do
    /usr/bin/time \
      -f "decoder=${decoder} run=${run} elapsed_s=%e user_s=%U sys_s=%S max_rss_kb=%M" \
      -o ".tmp/bench/time_${decoder}_${run}.txt" \
      ".tmp/target/release/examples/decode_bench_${decoder}" 20 \
      > /dev/null \
      2> ".tmp/bench/stderr_${decoder}_${run}.txt"
  done
done

cat .tmp/bench/time_*.txt
grep -h "decoded_bytes" .tmp/bench/stderr_*.txt
```

Benchmark workload:

- 29 pinned `.jxr` fixtures under `fixtures/data`.
- 20 full decode iterations per run.
- 3 measured runs per decoder.
- Decoded byte parity: both decoders produced `646,520,320` bytes per run.

Measured with `/usr/bin/time` on this host:

| Decoder | Avg wall time | Avg user time | Avg max RSS | Throughput |
| --- | ---: | ---: | ---: | ---: |
| Upstream `jpegxr` wrapper | 7.64 s | 7.42 s | 11,201 KB | 80.7 MiB/s |
| Translated Rust decoder | 6.85 s | 6.67 s | 12,061 KB | 90.0 MiB/s |

On this fixture set the translated decoder was `1.115x` faster wall-clock
(`11.5%`) and used 860 KB more max RSS (`7.7%`). "Upstream" here means the
local `jpegxr` `0.3.1` dev-dependency used by `tests/real_fixtures.rs` as the
decode oracle, not a standalone C command-line binary.




## License

New BSD License (BSD)

