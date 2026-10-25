use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::pcnd;

#[test]
fn flatten_roundtrips_exact_composition_and_guards_existing_outputs_and_input() {
    let scratch = Scratch::new("flatten");
    let input = scratch.path("source.pcnd");
    let output = scratch.path("flat.pcnd");
    let mut document = new_document(9, 7, Some(Rgba([40, 80, 120, 127])), 0, POLICY).unwrap();
    document.title = Some("Layered source".into());
    let overlay = new_document(6, 5, Some(Rgba([200, 30, 40, 128])), 0, POLICY).unwrap();
    let mut layer = overlay.layers[&overlay.roots[0]].clone();
    layer.origin = (-1, 2);
    layer.opacity = 173;
    document.roots.push(layer.id);
    document.layers.insert(layer.id, layer);
    let expected = paintcraft_render::render_flat(&document).unwrap();
    let bytes = pcnd::write(&document).unwrap();
    fs::write(&input, &bytes).unwrap();
    let result = run(&["flatten", &input, "-o", &output, "--json"]);
    assert_eq!(result.code, 0, "{}", result.stderr);
    let flat = pcnd::read(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(flat.document_id, document.document_id);
    assert_eq!(flat.title, document.title);
    assert_eq!(flat.roots.len(), 1);
    assert_eq!(paintcraft_render::render_flat(&flat).unwrap(), expected);
    assert_eq!(fs::read(&input).unwrap(), bytes);
    let output_bytes = fs::read(&output).unwrap();
    assert_ne!(run(&["flatten", &input, "-o", &output]).code, 0);
    assert_eq!(fs::read(&output).unwrap(), output_bytes);
    assert_ne!(
        run(&[
            "flatten",
            &input,
            "-o",
            &output,
            "--overwrite",
            "--expect-input-sha256",
            &"0".repeat(64)
        ])
        .code,
        0
    );
    assert_eq!(fs::read(&output).unwrap(), output_bytes);
    let replaced = run(&["flatten", &input, "-o", &input, "--overwrite", "--json"]);
    assert_eq!(replaced.code, 0, "{}", replaced.stderr);
    assert_eq!(
        fs::read(&input).unwrap(),
        output_bytes,
        "same source produces deterministic output"
    );
}
