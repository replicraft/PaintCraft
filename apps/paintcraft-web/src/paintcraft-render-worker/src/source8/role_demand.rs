//! Pure defining demands for the upcoming Main-owned Source8 service adapter.
use super::{frame_image, frame_inputs, source8_reader_control_bytes};
use crate::blend_service::LayerBlendService;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::SourceMetadata;
use paintcraft_formats::pcnd::{
    BlendSourceDemand, PortableSource8MaterializerControlBytes, layer_blend_appearance,
    portable_source8_materializer_control_bytes,
};
use paintcraft_raster::{PORTABLE_PIXEL_CONTROL_BYTES, ReservedDecodeAdmission};
use paintcraft_render::{
    blend::metadata::UnboundRootedFrameWorkspace, display::DisplayAccumulator,
    noise_work::NoiseAdmission,
};
use std::mem::size_of;
const EDGE: u64 = 64;
const JOB: u64 = 64 << 20;
/// Scalar demand only. The array order is the protocol8 E/P/M/A/I/C/Q/K/W/F/O/H.
/// All source/hash/owner and original-factory checks still belong to the consumer.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source8RoleDemand {
    pub roles: [u64; 12],
    pub paired_noise_bytes: u64,
    pub total_bytes: u64,
    pub read_control_bytes: u64,
    pub display_control_bytes: u64,
    pub staged_pixels: u64,
    pub compiled_profile: [u8; 32],
}
/// Caller-owned containing controls and formation work; never an admission.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source8RoleQueryCost {
    pub control_bytes: u64,
    pub formation_work: u64,
}
fn limited() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source8 role demand extent")
}
fn sum(values: &[u64]) -> Result<u64> {
    values
        .iter()
        .try_fold(0u64, |a, n| a.checked_add(*n).ok_or_else(limited))
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(limited)
}
/// Defining logical inline controls for the complete query and its returns.
///
/// The caller admits this declaration in its ORIGINAL Main operation before
/// calling the role query, and prepays formation_work there. This cost discovery
/// retains the existing bounded pure-quote bootstrap exclusion; it allocates no
/// budget, owner, factory or job. Logical inline sizes do not intercept allocator
/// headers, RSS, callback allocations or arbitrary escaped Error message heaps.
///
/// # Errors
/// Returns ResourceLimit on checked target-size/work overflow.
pub fn source8_role_query_cost() -> Result<Source8RoleQueryCost> {
    let controls = sum(&[
        4 * size_of::<Source8RoleDemand>() as u64,
        4 * size_of::<Result<Source8RoleDemand>>() as u64,
        2 * size_of::<BlendSourceDemand>() as u64,
        2 * size_of::<UnboundRootedFrameWorkspace>() as u64,
        2 * size_of::<super::Source8ReaderControlBytes>() as u64,
        2 * size_of::<PortableSource8MaterializerControlBytes>() as u64,
        2 * size_of::<paintcraft_raster::ReservedDecodeControlQuote>() as u64,
        2 * size_of::<paintcraft_document::source_metadata::ReservedSourceMetadataLayout>() as u64,
        2 * size_of::<Result<super::Source8ReaderControlBytes>>() as u64,
        2 * size_of::<Result<PortableSource8MaterializerControlBytes>>() as u64,
        2 * size_of::<Result<paintcraft_raster::ReservedDecodeControlQuote>>() as u64,
        2 * size_of::<Result<paintcraft_document::source_metadata::ReservedSourceMetadataLayout>>()
            as u64,
        2 * size_of::<paintcraft_render::noise_work::NoiseControlQuote>() as u64,
        2 * size_of::<Result<paintcraft_render::noise_work::NoiseControlQuote>>() as u64,
        2 * size_of::<Result<[u64; 3]>>() as u64,
        2 * size_of::<Result<[u8; 32]>>() as u64,
        2 * size_of::<[u64; 3]>() as u64,
        2 * size_of::<[u64; 12]>() as u64,
        16 * size_of::<Result<u64>>() as u64,
        16 * size_of::<u64>() as u64,
        8 * size_of::<&u8>() as u64,
        16 * size_of::<&[u64]>() as u64,
        2 * size_of::<Source8RoleQueryCost>() as u64,
        2 * size_of::<Result<Source8RoleQueryCost>>() as u64,
    ])?;
    Ok(Source8RoleQueryCost {
        control_bytes: controls,
        formation_work: mul(controls, 2)?.checked_add(16384).ok_or_else(limited)?,
    })
}
/// Complete serial read/display/service demands from SAME-target facts.
///
/// This parses no source and grants no capacity. `source` must come from the
/// actual counted source; `workspace` must describe the SAME immutable Document.
/// Their public scalar forms do not authenticate that relation: the trusted Main
/// consumer must retain and reconcile the original source/hash/owner binding.
/// Native facts cannot fund a wasm32 consumer. Future region/descriptor policies
/// require reviewing the defining Render bound before reusing this forecast.
///
/// The host owns/prepays `source8_role_query_cost` before this call. No budget,
/// callback, counter, TLS context or factory is captured/created. Every original
/// role must later be reserved exactly and checked by the real reader/service.
/// E is encoded data only; stage owner/controls are H. Noise's paired declaration
/// is conservative additional overlap even though its resident sidecar is in W.
/// No observed allocation/usage, fixture grant or fresh pool enters this result.
///
/// # Errors
/// Returns ResourceLimit for incompatible target/noise/geometry facts, checked
/// overflow, unsupported defining layouts, or a full role union above64MiB.
pub fn source8_role_demand_checked(
    source: &BlendSourceDemand,
    workspace: &UnboundRootedFrameWorkspace,
) -> Result<Source8RoleDemand> {
    let s = &source.retained;
    if source.pointer_bytes != size_of::<usize>() as u8
        || !source.stored_non_normal
        || source.stored_noise != workspace.requires_noise
        || source.encoded_bytes == 0
        || source.canvas.contains(&0)
        || s.layers() == 0
        || s.layers() > paintcraft_document::LAYER_COUNT_MAX as u64
        || s.attachment_count() > 1
        || source.p_peak < source.p_plan_census_peak.max(source.p_plan_probe_peak)
        || s.decoded_bytes() != mul(s.tiles(), 262144)?
        || source.wire_pixels > s.decoded_bytes() / 4
    {
        return Err(limited());
    }
    let reader = source8_reader_control_bytes()?;
    let material = portable_source8_materializer_control_bytes()?;
    let decoder = ReservedDecodeAdmission::control_quote()?;
    let icc = SourceMetadata::reserved_layout_quote()?;
    let q_fixed = if s.tiles() == 0 {
        0
    } else {
        decoder.fixed_bytes
    };
    if s.q_fixed() != q_fixed {
        return Err(limited());
    }
    let expected_i = if s.attachment_count() == 0 {
        if s.profile_bytes() != 0 {
            return Err(limited());
        }
        0
    } else {
        SourceMetadata::reserved_retained_quote(s.profile_bytes())?
    };
    if expected_i != s.icc_retained() {
        return Err(limited());
    }
    let coverage = sum(&[material.coverage, material.document_stream])?;
    let a = sum(&[
        s.a_retained(),
        coverage,
        if s.mask_present() == 0 {
            0
        } else {
            paintcraft_raster::coverage_mask::CANONICAL_A8_CONTROL_BYTES
        },
    ])?;
    let pixel = if s.tiles() == 0 {
        0
    } else {
        sum(&[
            q_fixed,
            material.pixels,
            PORTABLE_PIXEL_CONTROL_BYTES,
            s.scratch_bytes(),
        ])?
    };
    let adoption = sum(&[
        q_fixed,
        layer_blend_appearance::transient_quote(
            usize::try_from(s.layers()).map_err(|_| limited())?,
        )?,
    ])?;
    let icc_phase = if s.attachment_count() == 0 {
        0
    } else {
        icc.constructor_control_bytes
    };
    let read = sum(&[
        reader.entry,
        reader.assembly,
        icc_phase.max(pixel).max(adoption),
    ])?;
    let cells = mul(EDGE, EDGE)?;
    let display = sum(&[
        reader.entry,
        super::owner::display_outer_control_bytes(),
        mul(cells, size_of::<DisplayAccumulator>() as u64)?,
        q_fixed,
    ])?;
    let native_cells = mul(
        u64::from(source.canvas[0]).min(EDGE),
        u64::from(source.canvas[1]).min(EDGE),
    )?;
    if workspace.display_cells != native_cells
        || workspace.native_rgba_bytes != mul(native_cells, 4)?
        || workspace.display_accumulator_bytes
            != mul(native_cells, size_of::<DisplayAccumulator>() as u64)?
    {
        return Err(limited());
    }
    let paired = if source.stored_noise {
        NoiseAdmission::control_quote()?.counter_bytes
    } else {
        0
    };
    let render = sum(&[
        workspace.prepared_retained_bytes,
        workspace.spatial_workspace_bytes,
        workspace.blend_context_bytes,
        workspace.rooted_witness_bytes,
        workspace.seed_witness_bytes,
    ])?;
    let w = sum(&[paired, workspace.preparation_peak_bytes.max(render)])?;
    if workspace.noise_sidecar_bytes != paired || workspace.workspace_bytes != w {
        return Err(limited());
    }
    let h = sum(&LayerBlendService::control_quote()?)?;
    let roles = [
        source.encoded_bytes,
        source.p_peak,
        sum(&[s.m_struct(), s.m_tile(), s.m_cov(), s.m_icc()])?,
        a,
        expected_i,
        read.max(display),
        s.decoded_bytes(),
        reader.retained_carriers,
        w,
        frame_inputs::CONTROL_BYTES,
        mul(2, sum(&[mul(cells, 4)?, frame_image::RETAINED_CONTROL])?)?,
        h,
    ];
    let total = sum(&[sum(&roles)?, paired])?;
    if total > JOB {
        return Err(limited());
    }
    Ok(Source8RoleDemand {
        roles,
        paired_noise_bytes: paired,
        total_bytes: total,
        read_control_bytes: read,
        display_control_bytes: display,
        staged_pixels: s.decoded_bytes() / 4,
        compiled_profile: LayerBlendService::compiled_profile()?,
    })
}
#[cfg(test)]
#[path = "role_demand/tests.rs"]
mod tests;
