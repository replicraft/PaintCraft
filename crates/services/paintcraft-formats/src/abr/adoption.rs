//! Fail-closed adoption into PaintCraft's explicit native-grid static renderer.
//! Source resources stay immutable and retained; no preset becomes a stock nib.
use super::*;
use ag_psd::{
    abr::{Brush, BrushShape},
    descriptor::{Descriptor, DescriptorValue},
    reader::PsdReader,
};
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_raster::sampled_brush::{self, SampledTip};
use std::sync::Arc;

/// Frontend assessment inventory allowance; assessment revisits source descriptors.
pub const PRESETS_MAX: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticTipPolicy {
    NativeGridV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedFeature {
    ComputedTip,
    DynamicTip,
    MissingSample,
    AmbiguousSampleId,
    HighDepthMask,
    SizeOverride,
    Rotation,
    Roundness,
    Flip,
    DisabledSpacing,
    SpacingRange,
    OuterSpacing,
    ShapeDynamics,
    PressureDynamics,
    Scatter,
    Texture,
    DualBrush,
    ColorDynamics,
    Transfer,
    BrushPose,
    WetEdges,
    Noise,
    ProtectTexture,
    Interpretation,
    ToolOptions,
    SizePreference,
    UnrecognizedDescriptor,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequiredConversion {
    QuantizedSpacing,
}
#[derive(Clone, Debug)]
pub struct Assessment {
    unsupported: Vec<UnsupportedFeature>,
    conversions: Vec<RequiredConversion>,
    sample: Option<usize>,
    dimensions: Option<(u32, u32)>,
    spacing_q8: Option<u32>,
}
impl Assessment {
    pub fn unsupported(&self) -> &[UnsupportedFeature] {
        &self.unsupported
    }
    pub fn required_conversions(&self) -> &[RequiredConversion] {
        &self.conversions
    }
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions
    }
    pub fn spacing_q8(&self) -> Option<u32> {
        self.spacing_q8
    }
    pub fn qualified(&self) -> bool {
        self.unsupported.is_empty() && self.sample.is_some() && self.spacing_q8.is_some()
    }
    fn refuse(&mut self, feature: UnsupportedFeature) {
        if !self.unsupported.contains(&feature) {
            self.unsupported.push(feature);
        }
    }
}
#[derive(Clone, Debug)]
pub struct AdoptedPreset {
    source: Arc<Library>,
    preset: usize,
    tip: SampledTip,
    spacing_q8: u32,
    policy: StaticTipPolicy,
}
impl AdoptedPreset {
    pub fn source(&self) -> &Arc<Library> {
        &self.source
    }
    pub fn preset_index(&self) -> usize {
        self.preset
    }
    pub fn tip(&self) -> &SampledTip {
        &self.tip
    }
    pub fn spacing_q8(&self) -> u32 {
        self.spacing_q8
    }
    pub fn policy(&self) -> StaticTipPolicy {
        self.policy
    }
}

pub fn assess(
    source: &Library,
    preset: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Assessment> {
    check()?;
    let brush = source.brushes.get(preset).ok_or_else(|| {
        Error::new(
            ErrorCode::InputSchema,
            "ABR preset index is outside its resource",
        )
    })?;
    let mut result = Assessment {
        unsupported: Vec::new(),
        conversions: Vec::new(),
        sample: None,
        dimensions: None,
        spacing_q8: None,
    };
    classify_features(brush, &mut result);
    if !known_descriptor(source, preset, &mut result, check)? {
        result.refuse(UnsupportedFeature::UnrecognizedDescriptor);
    }
    let BrushShape::Sampled {
        sampled_data,
        size,
        angle,
        roundness,
        spacing_on,
        spacing,
        flip_x,
        flip_y,
        ..
    } = &brush.shape
    else {
        result.refuse(if matches!(brush.shape, BrushShape::Computed { .. }) {
            UnsupportedFeature::ComputedTip
        } else {
            UnsupportedFeature::DynamicTip
        });
        return Ok(result);
    };
    let matches: Vec<_> = source
        .samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.id == *sampled_data)
        .collect();
    let [(index, sample)] = matches.as_slice() else {
        result.refuse(if matches.is_empty() {
            UnsupportedFeature::MissingSample
        } else {
            UnsupportedFeature::AmbiguousSampleId
        });
        return Ok(result);
    };
    result.sample = Some(*index);
    let width = (sample.bounds[3] - sample.bounds[1]) as u32;
    let height = (sample.bounds[2] - sample.bounds[0]) as u32;
    result.dimensions = Some((width, height));
    if !matches!(sample.alpha, Alpha::U8(_)) {
        result.refuse(UnsupportedFeature::HighDepthMask);
    }
    if *size != f64::from(width.max(height)) {
        result.refuse(UnsupportedFeature::SizeOverride);
    }
    if angle.rem_euclid(360.0) != 0.0 {
        result.refuse(UnsupportedFeature::Rotation);
    }
    if *roundness != 1.0 {
        result.refuse(UnsupportedFeature::Roundness);
    }
    if *flip_x || *flip_y {
        result.refuse(UnsupportedFeature::Flip);
    }
    if !spacing_on {
        result.refuse(UnsupportedFeature::DisabledSpacing);
    }
    let raw_spacing = size * spacing * 256.0;
    if !raw_spacing.is_finite()
        || raw_spacing < 1.0
        || raw_spacing > f64::from(sampled_brush::SPACING_Q8_MAX)
    {
        result.refuse(UnsupportedFeature::SpacingRange);
    } else {
        result.spacing_q8 = Some(raw_spacing.round() as u32);
        if raw_spacing.fract() != 0.0 {
            result
                .conversions
                .push(RequiredConversion::QuantizedSpacing);
        }
    }
    check()?;
    Ok(result)
}

/// Explicit policy: preserve R8 samples and native dimensions, anchor at
/// floor(width/2),floor(height/2), apply equally spaced Q8 translations and max
/// union, then let the existing mask operation apply color opacity once.
/// This does not emulate Photoshop dynamics or reinterpret higher precision.
pub fn adopt_native(
    source: Arc<Library>,
    preset: usize,
    policy: StaticTipPolicy,
    acknowledged: &[RequiredConversion],
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<AdoptedPreset> {
    let assessment = assess(&source, preset, check)?;
    if !assessment.qualified() {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            format!(
                "ABR preset has unsupported static-tip features: {:?}",
                assessment.unsupported
            ),
        ));
    }
    if assessment
        .conversions
        .iter()
        .any(|loss| !acknowledged.contains(loss))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "ABR spacing quantization requires explicit acknowledgement",
        ));
    }
    let sample = &source.samples[assessment.sample.unwrap()];
    let Alpha::U8(alpha) = &sample.alpha else {
        return Err(unsupported("ABR static tip requires exact R8 coverage"));
    };
    let (width, height) = assessment.dimensions.unwrap();
    let tip = SampledTip::from_alpha(width, height, (width / 2, height / 2), alpha, budget, check)?;
    check()?;
    Ok(AdoptedPreset {
        source,
        preset,
        tip,
        spacing_q8: assessment.spacing_q8.unwrap(),
        policy,
    })
}

fn classify_features(brush: &Brush, result: &mut Assessment) {
    for (enabled, feature) in [
        (
            brush.shape_dynamics.is_some(),
            UnsupportedFeature::ShapeDynamics,
        ),
        (brush.scatter.is_some(), UnsupportedFeature::Scatter),
        (brush.texture.is_some(), UnsupportedFeature::Texture),
        (brush.dual_brush.is_some(), UnsupportedFeature::DualBrush),
        (
            brush.color_dynamics.is_some(),
            UnsupportedFeature::ColorDynamics,
        ),
        (brush.transfer.is_some(), UnsupportedFeature::Transfer),
        (brush.brush_pose.is_some(), UnsupportedFeature::BrushPose),
        (brush.wet_edges, UnsupportedFeature::WetEdges),
        (brush.noise, UnsupportedFeature::Noise),
        (
            brush.protect_texture == Some(true),
            UnsupportedFeature::ProtectTexture,
        ),
        (
            brush.interpretation == Some(true),
            UnsupportedFeature::Interpretation,
        ),
        (
            brush.tool_options.is_some(),
            UnsupportedFeature::ToolOptions,
        ),
        (!brush.use_brush_size, UnsupportedFeature::SizePreference),
        (brush.spacing != 1.0, UnsupportedFeature::OuterSpacing),
    ] {
        if enabled {
            result.refuse(feature);
        }
    }
    if brush.shape_dynamics.as_ref().is_some_and(|d| {
        [&d.size_dynamics, &d.angle_dynamics, &d.roundness_dynamics]
            .iter()
            .any(|v| {
                v.control.contains("pen ")
                    || v.control.contains("stylus")
                    || v.control.contains("rotation")
            })
    }) {
        result.refuse(UnsupportedFeature::PressureDynamics);
    }
}

fn known_descriptor(
    source: &Library,
    mut index: usize,
    assessment: &mut Assessment,
    check: &dyn Fn() -> Result<()>,
) -> Result<bool> {
    for section in &source.sections {
        if section.kind != SectionKind::Descriptors {
            continue;
        }
        check()?;
        // Library construction already preflighted the entire bounded tree.
        let mut reader = PsdReader::new(&section.payload, None, None);
        let root = ag_psd::descriptor::read_version_and_descriptor(&mut reader)
            .map_err(|e| invalid(&format!("ABR descriptor inspection failed: {e}")))?;
        check()?;
        let Some(DescriptorValue::List(items)) = root.get("Brsh") else {
            continue;
        };
        for value in items {
            let DescriptorValue::Descriptor(brush) = value else {
                continue;
            };
            if index != 0 {
                index -= 1;
                continue;
            }
            classify_raw_flags(brush, assessment);
            let root_known =
                root.items.iter().all(|(key, _)| key == "Brsh") && root.items.len() == 1;
            return Ok(root_known && known_brush(brush));
        }
    }
    Ok(false)
}
fn known_brush(brush: &Descriptor) -> bool {
    if brush.class_id != "brush" || brush.items.len() > 32 {
        return false;
    }
    for (index, (key, value)) in brush.items.iter().enumerate() {
        if brush.items[..index].iter().any(|(old, _)| old == key) {
            return false;
        }
        let known = match (key.as_str(), value) {
            ("Nm  ", DescriptorValue::Text(_)) => true,
            ("Brsh", DescriptorValue::Descriptor(shape)) => known_shape(shape),
            ("Spcn", DescriptorValue::UnitDouble(v)) => v.units == "Percent",
            (
                "Wtdg" | "Nose" | "useBrushSize" | "protectTexture" | "interpretation"
                | "useTipDynamics" | "useScatter" | "useTexture" | "useColorDynamics"
                | "usePaintDynamics" | "useBrushPose",
                DescriptorValue::Boolean(_),
            ) => true,
            _ => false,
        };
        if !known {
            return false;
        }
    }
    true
}
fn known_shape(shape: &Descriptor) -> bool {
    if shape.class_id != "sampledBrush"
        || shape.items.len() > 16
        || ["Dmtr", "Angl", "Rndn", "Spcn", "Intr", "sampledData"]
            .iter()
            .any(|key| shape.get(key).is_none())
    {
        return false;
    }
    shape.items.iter().enumerate().all(|(index, (key, value))| {
        !shape.items[..index].iter().any(|(old, _)| old == key)
            && match (key.as_str(), value) {
                ("Nm  " | "sampledData", DescriptorValue::Text(_)) => true,
                ("Dmtr", DescriptorValue::UnitDouble(v)) => v.units == "Pixels",
                ("Angl", DescriptorValue::UnitDouble(v)) => v.units == "Angle",
                ("Rndn" | "Spcn", DescriptorValue::UnitDouble(v)) => v.units == "Percent",
                ("Intr" | "flipX" | "flipY", DescriptorValue::Boolean(_)) => true,
                _ => false,
            }
    })
}

// Some upstream fields are conditional (e.g. useTexture with a missing Txtr
// yields None). Never interpret that lossy convenience-model omission as off.
fn classify_raw_flags(brush: &Descriptor, result: &mut Assessment) {
    for (key, feature) in [
        ("useTipDynamics", UnsupportedFeature::ShapeDynamics),
        ("useScatter", UnsupportedFeature::Scatter),
        ("useTexture", UnsupportedFeature::Texture),
        ("useColorDynamics", UnsupportedFeature::ColorDynamics),
        ("usePaintDynamics", UnsupportedFeature::Transfer),
        ("useBrushPose", UnsupportedFeature::BrushPose),
    ] {
        if matches!(brush.get(key), Some(DescriptorValue::Boolean(true))) {
            result.refuse(feature);
        }
    }
    if contains_pressure(brush) {
        result.refuse(UnsupportedFeature::PressureDynamics);
    }
}
fn contains_pressure(value: &Descriptor) -> bool {
    value.items.iter().any(|(key, value)| match value {
        // Selected backend's documented control table: pressure/tilt/wheel,
        // and initial/current rotation. Unknown controls fail known_brush.
        DescriptorValue::Integer(v) if key == "bVTy" => matches!(v, 2 | 3 | 4 | 7 | 8),
        DescriptorValue::Descriptor(value) => contains_pressure(value),
        _ => false,
    })
}

#[cfg(test)]
mod tests;
