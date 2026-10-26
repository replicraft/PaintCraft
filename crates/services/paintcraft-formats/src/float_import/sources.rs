use super::*;
#[cfg(feature = "hdr-codec")]
pub fn prepare_hdr(
    bytes: &[u8],
    pixels: u64,
    settings: Settings,
    budget: SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    let _scratch = scratch(&budget)?;
    let admitted = crate::radiance::inspect(bytes, pixels, check)?;
    let header = admitted.header();
    let binding = Binding::new(
        admitted.sha256(),
        header.width,
        header.height,
        header.color_metadata(),
        precision::SourceAlpha::Opaque,
        settings,
    )?;
    let (source, metadata) = source(
        "radiance-rgbe",
        bytes.len(),
        ([0, 0], header.pixel_aspect),
        "as-stored-rgbe:E=0-black;E>0:M*2^(E-136)",
        &bytes[..header.data_offset],
        vec![
            ("R", "shared-exponent-rgbe"),
            ("G", "shared-exponent-rgbe"),
            ("B", "shared-exponent-rgbe"),
        ],
        &budget,
    )?;
    let mut losses = vec!["hdr-rgbe-lower-edge-representative"];
    if header.has_noncolor_metadata() {
        losses.push("hdr-metadata-not-retained");
    }
    if header.pixel_aspect != 1.0 {
        losses.push("hdr-pixel-aspect-normalized");
    }
    let decoded = crate::radiance::decode::decode(&admitted, budget.clone(), check)?;
    finish(
        binding,
        source,
        metadata,
        decoded.samples(),
        losses,
        budget,
        check,
    )
}
#[cfg(feature = "exr-codec")]
pub fn prepare_exr(
    bytes: &[u8],
    pixels: u64,
    settings: Settings,
    budget: SharedBudget,
    check: &(dyn Fn() -> Result<()> + Sync),
) -> Result<Prepared> {
    let _scratch = scratch(&budget)?;
    let admitted = crate::exr::inspect(bytes, pixels, check)?;
    let header = admitted.header();
    let binding = Binding::new(
        admitted.sha256(),
        header.width,
        header.height,
        header.color_metadata(),
        header.source_alpha(),
        settings,
    )?;
    let aspect = f32::from_le_bytes(
        header
            .attributes
            .iter()
            .find(|a| a.name == "pixelAspectRatio")
            .unwrap()
            .value
            .try_into()
            .unwrap(),
    );
    let channels = header
        .channels
        .iter()
        .map(|c| {
            (
                match c.name {
                    "R" => "R",
                    "G" => "G",
                    "B" => "B",
                    _ => "A",
                },
                match c.sample_type {
                    crate::exr::SampleType::Half => "half",
                    crate::exr::SampleType::Float => "float",
                },
            )
        })
        .collect();
    let (source, metadata) = source(
        "openexr",
        bytes.len(),
        (header.origin, f64::from(aspect)),
        "decoder-as-stored-linear-float-no-alpha-unassociation",
        admitted.raw_header(),
        channels,
        &budget,
    )?;
    let mut losses = Vec::new();
    if header.origin != [0, 0] {
        losses.push("exr-origin-normalized");
    }
    if aspect != 1.0 {
        losses.push("hdr-pixel-aspect-normalized");
    }
    if header.attributes.iter().any(|a| match a.name {
        "channels" | "compression" | "dataWindow" | "displayWindow" | "lineOrder"
        | "pixelAspectRatio" | "chromaticities" | "colorInteropID" | "type" => false,
        "screenWindowCenter" => a
            .value
            .as_chunks::<4>()
            .0
            .iter()
            .any(|v| f32::from_le_bytes(*v) != 0.0),
        "screenWindowWidth" => a.value != 1.0f32.to_le_bytes(),
        _ => true,
    }) {
        losses.push("hdr-metadata-not-retained");
    }
    let decoded = crate::exr::decode::decode(&admitted, budget.clone(), check)?;
    finish(
        binding,
        source,
        metadata,
        decoded.samples(),
        losses,
        budget,
        check,
    )
}
