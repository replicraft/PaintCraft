use super::*;
use paintcraft_core::types::json;
use std::sync::atomic::{AtomicUsize, Ordering};
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/exr-fixtures/");
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{ROOT}{name}")).unwrap()
}
#[test]
fn selected_decoder_exact_samples_match_official_reference_and_keep_source_provenance() {
    let manifest = json::parse(&fixture("manifest.json")).unwrap();
    for case in manifest.get("fixtures").unwrap().as_array().unwrap() {
        let name = case.get("file").unwrap().as_str().unwrap();
        let source = fixture(name);
        let before = source.clone();
        let admitted = inspect(&source, PIXELS_MAX, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(64 << 20);
        let decoded =
            decode(&admitted, budget.clone(), &|| Ok(())).unwrap_or_else(|e| panic!("{name}: {e}"));
        let golden = fixture(&name.replace(".exr", ".f32le"));
        let expected: Vec<_> = golden
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v).to_bits())
            .collect();
        assert_eq!(
            decoded
                .samples()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected,
            "{name}"
        );
        assert_eq!(decoded.source_sha256(), admitted.sha256());
        assert_eq!(decoded.origin(), admitted.header.origin);
        assert_eq!(
            decoded.alpha(),
            case.get("alpha").unwrap().as_bool().unwrap()
        );
        assert_eq!(source, before);
        assert_eq!(budget.usage().reserved, decoded.samples().len() as u64 * 4);
        drop(decoded);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn output_denial_read_cancel_and_corrupt_compressed_payload_are_private_failures() {
    let source = fixture("mixed-rgba-zip.exr");
    let admitted = inspect(&source, PIXELS_MAX, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(1);
    let error = decode(&admitted, budget.clone(), &|| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(
        error.allocation_denial().unwrap().class(),
        AllocationClass::DecodedTiles
    );
    assert_eq!(budget.usage().reserved, 0);
    for at in [1, 2, 10, 40] {
        let budget = SharedBudget::new(64 << 20);
        let calls = AtomicUsize::new(0);
        let result = decode(&admitted, budget.clone(), &|| {
            let call = calls.fetch_add(1, Ordering::Relaxed) + 1;
            if call == at {
                Err(Error::new(ErrorCode::OperationCancelled, "decode callback"))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::OperationCancelled,
            "check{at}"
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    let mut input = Bytes {
        bytes: &source,
        at: 8,
    };
    loop {
        let name = input.text().unwrap();
        if name.is_empty() {
            break;
        }
        input.text().unwrap();
        let len = input.i32().unwrap();
        input.take(len as usize).unwrap();
    }
    let offset = u64::from_le_bytes(source[input.at..input.at + 8].try_into().unwrap()) as usize;
    let size = i32::from_le_bytes(source[offset + 4..offset + 8].try_into().unwrap()) as usize;
    let mut corrupt = source.clone();
    corrupt[offset + 8..offset + 8 + size].fill(0xff);
    let admitted = inspect(&corrupt, PIXELS_MAX, &|| Ok(())).unwrap();
    let budget = SharedBudget::new(64 << 20);
    assert_eq!(
        decode(&admitted, budget.clone(), &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CodecFailed
    );
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn unfamiliar_named_typed_metadata_is_validated_before_any_decoder_allocation() {
    let source = fixture("half-rgb-none.exr");
    let mut cases = vec![
        ("preview", vec![255; 8]),
        ("preview", vec![0; 7]),
        ("stringvector", i32::MAX.to_le_bytes().to_vec()),
        ("stringvector", (-1i32).to_le_bytes().to_vec()),
        ("bytes", u32::MAX.to_le_bytes().to_vec()),
        ("chlist", vec![0]),
        ("m44f", vec![0; 63]),
        (
            "box2i",
            [i32::MIN, 0, i32::MAX, 0]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect(),
        ),
    ];
    for (kind, size) in [
        ("box2i", 16),
        ("box2f", 16),
        ("int", 4),
        ("float", 4),
        ("double", 8),
        ("rational", 8),
        ("timecode", 8),
        ("v2i", 8),
        ("v2f", 8),
        ("v3i", 12),
        ("v3f", 12),
        ("chromaticities", 32),
        ("compression", 1),
        ("envmap", 1),
        ("keycode", 28),
        ("lineOrder", 1),
        ("m33f", 36),
        ("m44f", 64),
        ("tiledesc", 9),
    ] {
        cases.push((kind, vec![0; size - 1]));
    }
    for (kind, data) in cases {
        // Insert hostile typed metadata into a real source and repair only
        // the bounded container offsets, so sealing cannot be bypassed.
        let mut input = Bytes {
            bytes: &source,
            at: 8,
        };
        loop {
            let name = input.text().unwrap();
            if name.is_empty() {
                break;
            }
            input.text().unwrap();
            let count = input.i32().unwrap();
            input.take(count as usize).unwrap();
        }
        let table = input.at;
        let mut attribute = b"unfamiliar\0".to_vec();
        attribute.extend_from_slice(kind.as_bytes());
        attribute.push(0);
        attribute.extend_from_slice(&(data.len() as i32).to_le_bytes());
        attribute.extend_from_slice(&data);
        let shift = attribute.len();
        let mut hostile = source.clone();
        hostile.splice(table - 1..table - 1, attribute);
        for index in 0..2 {
            let old = table + index * 8;
            let offset = u64::from_le_bytes(source[old..old + 8].try_into().unwrap());
            hostile[old + shift..old + shift + 8]
                .copy_from_slice(&(offset + shift as u64).to_le_bytes());
        }
        let admitted = inspect(&hostile, PIXELS_MAX, &|| Ok(())).unwrap();
        let budget = SharedBudget::new(0);
        let error = decode(&admitted, budget.clone(), &|| Ok(())).unwrap_err();
        assert_ne!(
            error.code,
            ErrorCode::ResourceLimit,
            "{kind} must fail metadata validation before allocator admission"
        );
        assert_eq!(budget.usage().reserved, 0);
    }
}
struct ExitProbe {
    budget: SharedBudget,
    held: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for ExitProbe {
    fn drop(&mut self) {
        self.held.store(
            self.budget.usage().reserved >= 4 * (1 << 20),
            Ordering::SeqCst,
        );
    }
}
thread_local! {static EXIT_PROBE:RefCell<Option<ExitProbe>>=const{RefCell::new(None)};}
#[test]
fn private_scratch_charge_survives_thread_exit_on_success_cancel_and_unwind() {
    let source = fixture("mixed-rgba-zip.exr");
    let admitted = inspect(&source, PIXELS_MAX, &|| Ok(())).unwrap();
    let caller = std::thread::current().id();
    for mode in [0, 1, 2] {
        let budget = SharedBudget::new(64 << 20);
        let held = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen = std::sync::atomic::AtomicBool::new(false);
        let result = decode(&admitted, budget.clone(), &|| {
            if std::thread::current().id() != caller && !seen.swap(true, Ordering::SeqCst) {
                EXIT_PROBE.with(|slot| {
                    *slot.borrow_mut() = Some(ExitProbe {
                        budget: budget.clone(),
                        held: held.clone(),
                    })
                });
                if mode == 1 {
                    return Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "thread exit cancellation",
                    ));
                }
                assert_ne!(mode, 2, "qualification callback panic");
            }
            Ok(())
        });
        assert!(seen.load(Ordering::SeqCst));
        assert!(
            held.load(Ordering::SeqCst),
            "thread-local destructor must run before scratch charge releases"
        );
        match mode {
            0 => {
                drop(result.unwrap());
            }
            1 => assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled),
            _ => assert_eq!(result.unwrap_err().code, ErrorCode::CodecFailed),
        }
        assert_eq!(budget.usage().reserved, 0);
    }
}
