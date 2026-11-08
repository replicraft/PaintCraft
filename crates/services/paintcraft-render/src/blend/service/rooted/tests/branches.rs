//! Public API branch controls with literals/prices frozen before this source.
use crate as render;
use paintcraft_core::types::{
    Error, ErrorCode, LayerBlendMode, Rect, Result, Rgba,
    filters::{BoxBlurV1, Descriptor, VibranceRgbV1},
    ids::deterministic_id,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack, LayerMask};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use render::{
    blend::{
        metadata::quote_unbound_rooted_call_checked,
        service::{self, Caller},
    },
    spatial::{WorkspaceAdmission, with_workspace},
};
use std::cell::Cell;
fn fail() -> Error {
    Error::new(ErrorCode::InvariantViolation, "rooted branch diagnostic")
}
fn require(value: bool, checks: &Cell<u64>) -> Result<()> {
    checks.set(checks.get() + 1);
    if value { Ok(()) } else { Err(fail()) }
}
fn scene(case: u32) -> Result<Document> {
    let direct = case == 1802 || case >= 1804;
    let (width, pixels): (u32, &[u8]) = if direct {
        (2, &[20, 115, 200, 255, 64, 16, 128, 255])
    } else {
        (3, &[0, 0, 0, 255, 90, 90, 90, 255, 240, 240, 240, 255])
    };
    let mut doc = paintcraft_document::new_document(
        width,
        1,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )?;
    let source = paintcraft_document::import_image(width, 1, pixels, "rooted-branch-source")?;
    let mut top = source.layers[&source.roots[0]].clone();
    top.id = deterministic_id(&["rooted-branch-top"]);
    top.blend_mode = LayerBlendMode::Multiply;
    top.opacity = 255;
    let mut descriptors = if direct {
        vec![Descriptor::VibranceRgbV1(VibranceRgbV1::new(100)?)]
    } else {
        vec![Descriptor::BoxBlurV1(BoxBlurV1::new(1)?)]
    };
    if case == 1803 {
        descriptors.push(Descriptor::InvertRgbV1);
    }
    top.filters = FilterStack::new(
        descriptors
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                FilterInstance::new(
                    deterministic_id(&["rooted-branch-filter", &i.to_string()]),
                    true,
                    d,
                )
            })
            .collect(),
    )?;
    if direct || case == 1803 {
        top.mask = Some(LayerMask::new(
            true,
            A8Mask::uniform(
                Rect::new(0, 0, i64::from(width), 1).ok_or_else(fail)?,
                128,
                &SharedBudget::new(1 << 20),
                Limits::default(),
                &|| Ok(()),
            )?,
        ));
    }
    doc.roots.push(top.id);
    doc.layers.insert(top.id, top);
    Ok(doc)
}
fn debt(used: &Cell<u64>, n: u64) -> Result<()> {
    used.set(
        used.get()
            .checked_add(n)
            .filter(|v| *v <= 1_000_000)
            .ok_or_else(fail)?,
    );
    Ok(())
}
pub(super) fn run(case: u32) -> Result<u64> {
    let checks = Cell::new(0);
    let doc = scene(case)?;
    let before = doc.clone();
    let f = if case == 1803 { 2 } else { 1 };
    let supplement = if f == 2 { 24690 } else { 24674 };
    let quote_paid = if f == 2 { 13010 } else { 12978 };
    let roi = if case == 1801 {
        Rect::new(1, 0, 1, 1).ok_or_else(fail)?
    } else {
        doc.canvas_rect()
    };
    let quoted = Cell::new(0u64);
    let quote = quote_unbound_rooted_call_checked(&doc, roi, &|| Ok(()), &|n| debt(&quoted, n))?;
    require(
        quoted.get() == quote_paid && quote.traversal_work == supplement,
        &checks,
    )?;
    let b = SharedBudget::new(4 << 20);
    let reservation = b.reserve_capacity(1 << 20)?;
    let scope = reservation.clone().scope(AllocationClass::SharedBudget);
    let a = WorkspaceAdmission::new(scope.clone(), 1 << 20, 1_000_000);
    let programs = service::prepare_programs_checked(&doc, Caller::Base(&a), &|| Ok(()), &|n| {
        a.reserve_work(n)
    })?;
    let retained = scope.footprint().live;
    if case >= 1804 {
        let initial = a.work_used();
        if case == 1804 {
            let error = service::render_rooted_call_checked(&doc, roi, &programs, &|| {
                if a.work_used() >= initial + 8370 {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "rooted branch cancel",
                    ))
                } else {
                    Ok(())
                }
            })
            .err()
            .ok_or_else(fail)?;
            require(
                error.code == ErrorCode::OperationCancelled && a.work_used() == initial + 8370,
                &checks,
            )?;
        } else {
            let child = with_workspace(programs.workspace().clone(), || {
                render::spatial::capture_workspace_or_current(
                    || panic!("installed rooted diagnostic caller must prevent fallback"),
                    1 << 20,
                    initial + 8369,
                )
            })?;
            require(
                child.same_job(programs.workspace())
                    && child.work_used() == initial
                    && child.bytes_limit() == 1 << 20
                    && child.work_limit() == initial + 8369,
                &checks,
            )?;
            let error = with_workspace(child, || {
                service::render_rooted_call_checked(&doc, roi, &programs, &|| Ok(()))
            })
            .err()
            .ok_or_else(fail)?;
            require(
                error.code == ErrorCode::ResourceLimit && a.work_used() == initial + 8354,
                &checks,
            )?;
        }
        require(
            scope.footprint().live == retained && reservation.available() == (1 << 20) - retained,
            &checks,
        )?;
        // Retrying on the SAME original owner retains the first issued prefix.
        let paid = a.work_used();
        let mut output = scope.account();
        output.resize(8)?;
        let bytes = service::render_rooted_call_checked(&doc, roi, &programs, &|| Ok(()))?;
        output.resize(bytes.capacity() as u64)?;
        require(
            bytes == [129, 185, 235, 255, 158, 129, 197, 255] && a.work_used() > paid,
            &checks,
        )?;
        drop(bytes);
        drop(output);
    } else {
        let expected: &[u8] = match case {
            1800 => &[30, 30, 30, 255, 110, 110, 110, 255, 190, 190, 190, 255],
            1801 => &[110, 110, 110, 255],
            1802 => &[129, 185, 235, 255, 158, 129, 197, 255],
            1803 => &[240, 240, 240, 255, 200, 200, 200, 255, 160, 160, 160, 255],
            _ => return Err(fail()),
        };
        let mut work = [0; 2];
        for (index, rooted) in [false, true].into_iter().enumerate() {
            // Each comparison uses a genuine independent fixed-capacity owner.
            let c = WorkspaceAdmission::new(scope.clone(), 1 << 20, 1_000_000);
            let p = service::prepare_programs_checked(&doc, Caller::Base(&c), &|| Ok(()), &|n| {
                c.reserve_work(n)
            })?;
            let mut output = scope.account();
            output.resize(expected.len() as u64)?;
            let bytes = if rooted {
                service::render_rooted_call_checked(&doc, roi, &p, &|| Ok(()))
            } else {
                service::render_call_checked(&doc, roi, &p, &|| Ok(()))
            }?;
            output.resize(bytes.capacity() as u64)?;
            require(bytes == expected, &checks)?;
            work[index] = c.work_used();
            drop(bytes);
            drop(output);
            drop(p);
            require(scope.footprint().live == retained, &checks)?;
        }
        require(work[1].checked_sub(work[0]) == Some(supplement), &checks)?;
    }
    require(
        doc.state_id == before.state_id && doc.same_semantics(&before),
        &checks,
    )?;
    let paid = a.work_used();
    drop(programs);
    require(
        scope.footprint().live == 0 && reservation.available() == 1 << 20 && a.work_used() == paid,
        &checks,
    )?;
    drop(a);
    drop(scope);
    drop(reservation);
    require(b.usage().reserved == 0, &checks)?;
    Ok(checks.get())
}
#[test]
fn nonuniform_spatial_roi_direct_mask_and_mixed_branches_match_frozen_literals() {
    for case in 1800..=1803 {
        run(case).unwrap();
    }
}
#[test]
fn cancelled_and_refused_rooted_facts_keep_original_debt_and_scope_before_retry() {
    for case in 1804..=1805 {
        run(case).unwrap();
    }
}
