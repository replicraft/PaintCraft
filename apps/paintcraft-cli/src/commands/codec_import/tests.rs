use super::*;
use paintcraft_runtime::SharedBudget;
use std::fs;
#[test]
fn prepared_document_retains_charge_across_executor_drop_and_staged_publication() {
    let root = std::env::temp_dir().join(format!(
        "paintcraft-prepared-import-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&root).unwrap();
    let input = root.join("source.jxl");
    fs::write(&input, b"source").unwrap();
    let owner = super::super::codec_worker::owner();
    let identity = SourceIdentity::existing_destination(&input)
        .unwrap()
        .unwrap();
    let sha: [u8; 32] = Sha256::digest(b"source").into();
    let budget = SharedBudget::new(512 << 20);
    let mut executor = Executor::with_budget(
        Limits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 512 << 20,
        },
        budget.clone(),
    )
    .unwrap();
    let captured = input.clone();
    let charge = reservation(6).unwrap();
    let handle = executor
        .submit(
            charge,
            move |cancel| {
                Ok(Prepared {
                    document: paintcraft_document::import_image_checked(
                        1,
                        1,
                        &[17, 29, 91, 137],
                        &hex(&sha),
                        &|| cancel.check(),
                    )?,
                    identity,
                    input: captured,
                    length: 6,
                    sha,
                    owner,
                    cancel: cancel.clone(),
                    _lease: cancel.resource_lease().unwrap(),
                })
            },
            || {},
        )
        .unwrap();
    let prepared = loop {
        if let Some(value) = executor.poll_for(handle) {
            break value.unwrap();
        }
        std::thread::yield_now();
    };
    drop(executor);
    assert_eq!(budget.usage().reserved, charge);
    let output = Output {
        output: root.join("new.pcnd"),
        overwrite: false,
    };
    let mut foreign = owner;
    foreign.epoch += 1;
    assert!(prepared.save(&output, || Ok(foreign)).is_err());
    assert!(!output.output.exists());
    assert!(
        prepared
            .save(
                &Output {
                    output: input.clone(),
                    overwrite: true
                },
                || Ok(owner)
            )
            .is_err()
    );
    assert_eq!(fs::read(&input).unwrap(), b"source");
    let queries = std::cell::Cell::new(0);
    let staged = Output {
        output: root.join("stale-while-staging.pcnd"),
        overwrite: false,
    };
    let failed = prepared
        .save(&staged, || {
            let count = queries.get();
            queries.set(count + 1);
            Ok(if count == 0 { owner } else { foreign })
        })
        .unwrap();
    assert!(!failed.ok);
    assert!(!staged.output.exists());
    let alias = root.join("late-alias.pcnd");
    fs::hard_link(&input, &alias).unwrap();
    assert!(
        prepared
            .save(
                &Output {
                    output: alias.clone(),
                    overwrite: true
                },
                || Ok(owner)
            )
            .is_err()
    );
    assert_eq!(fs::read(alias).unwrap(), b"source");
    let written = prepared.save(&output, || Ok(owner)).unwrap();
    assert!(written.ok);
    let engine = written.engine.as_ref().unwrap();
    assert_eq!(
        engine.get("qualification").and_then(Value::as_str),
        Some("internal-native-only")
    );
    assert_eq!(
        engine.get("conversion").and_then(Value::as_str),
        Some("new-pcnd-copy")
    );
    assert_eq!(
        engine.get("public_reader_enabled").and_then(Value::as_bool),
        Some(false)
    );
    let document = pcnd::read(&fs::read(&output.output).unwrap()).unwrap();
    assert_eq!(document.canvas, (1, 1));
    assert!(document.same_semantics(&prepared.document));
    assert_eq!(budget.usage().reserved, charge);
    fs::write(&input, b"mutate").unwrap();
    assert!(
        prepared
            .save(
                &Output {
                    output: root.join("changed.pcnd"),
                    overwrite: false
                },
                || Ok(owner)
            )
            .is_err()
    );
    assert!(!root.join("changed.pcnd").exists());
    drop(prepared);
    assert_eq!(budget.usage().reserved, 0);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn padded_tile_declaration_covers_every_qualified_dimension() {
    let mut maximum = 0;
    for width in 1u64..=65536 {
        let height = 65536.min(wire::PIXELS_MAX / width);
        maximum = maximum.max(width.div_ceil(256) * height.div_ceil(256) * 256 * 256 * 4);
    }
    assert_eq!(maximum, DOCUMENT_BYTES);
}
