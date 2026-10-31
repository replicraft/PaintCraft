//! Actual native-ABI Export7 execution; literal colors are independent HSL results.
use super::*;
use paintcraft_core::types::filters::{ThresholdLumaV1, VibranceRgbV1};
use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};

fn document(codec: Codec, enabled: bool) -> Document {
    let mut doc = paintcraft_document::import_image(
        1,
        1,
        &[64, 16, 128, 255],
        "literal Vibrance Worker7 source",
    )
    .unwrap();
    let mut entries = vec![FilterInstance::new(
        Uuid::from_u128(51),
        enabled,
        Descriptor::VibranceRgbV1(VibranceRgbV1::new(100).unwrap()),
    )];
    if matches!(codec, Codec::Msp) {
        entries.push(FilterInstance::new(
            Uuid::from_u128(52),
            true,
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(30).unwrap()),
        ));
    }
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(entries).unwrap();
    doc
}
#[test]
fn actual_vibrance_worker7_direct_pixels_and_order_are_causal_and_preserve_source() {
    for codec in [Codec::Dds, Codec::Msp] {
        for enabled in [false, true] {
            let doc = document(codec, enabled);
            let original = pcnd::write(&doc).unwrap();
            let request = packet(&doc, codec);
            let Outcome::Success(encoded, preview, losses) =
                execute(&request, codec, &|| Ok(())).unwrap()
            else {
                panic!("valid Vibrance refused")
            };
            // Original luma is34.29 (>30); V+100 luma is26.15 (<30).
            // Thus MSP white→black proves Vibrance executes BEFORE Threshold.
            let expected = match (codec, enabled) {
                (Codec::Dds, true) => [62, 4, 140, 255],
                (Codec::Dds, false) => [64, 16, 128, 255],
                (Codec::Msp, true) => [0, 0, 0, 255],
                (Codec::Msp, false) => [255; 4],
            };
            assert_eq!(preview, expected);
            let decoded = match codec {
                Codec::Dds => {
                    dds::decode_top_mip_checked(&encoded, 1, &|| Ok(()))
                        .unwrap()
                        .rgba
                }
                Codec::Msp => paintcraft_formats::msp::decode(&encoded, &|| Ok(()))
                    .unwrap()
                    .rgba(&|| Ok(()))
                    .unwrap(),
            };
            assert_eq!(decoded, expected);
            assert!(losses.iter().any(|loss| loss == filter_bake::BAKED_REASON));
            assert!(
                !losses
                    .iter()
                    .any(|loss| loss == metadata_policy::OMITTED_REASON)
            );
            assert_eq!(pcnd::write(&doc).unwrap(), original);
            assert_eq!(
                doc.layers[&doc.roots[0]].raster.get(0, 0).0,
                [64, 16, 128, 255]
            );
        }
    }
}
#[test]
fn vibrance_worker7_requires_bake_authority_and_propagates_initial_mid_final_cancellation() {
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = document(codec, true);
        let source = pcnd::write(&doc).unwrap();
        let target = metadata_policy::Target::new(
            match codec {
                Codec::Dds => Family::Dds,
                Codec::Msp => Family::Msp,
            },
            Operation::ExportFlat,
        );
        let mut refusal = policies(&doc, codec);
        refusal.filter_bake =
            filter_bake::Policy::capture(&doc, target, filter_bake::Intent::Refuse, &|| Ok(()))
                .unwrap();
        let failure = match codec {
            Codec::Dds => {
                d::SourcePlan::capture_with_policies(&doc, options(), refusal).unwrap_err()
            }
            Codec::Msp => m::SourcePlan::capture_with_policies(&doc, refusal).unwrap_err(),
        };
        assert_eq!(failure.code, ErrorCode::CapabilityUnsupported);
        let original = packet(&doc, codec);
        let mut revoked = original.clone();
        revoked[122] = 0;
        assert_eq!(
            error(execute(&revoked, codec, &|| Ok(()))).code,
            ErrorCode::CapabilityUnsupported
        );
        let count = Cell::new(0);
        assert!(matches!(
            execute(&original, codec, &|| {
                count.set(count.get() + 1);
                Ok(())
            })
            .unwrap(),
            Outcome::Success(..)
        ));
        let total = count.get();
        assert!(total > 3);
        for stop in [1, total / 2, total] {
            let at = Cell::new(0);
            let failure = error(execute(&original, codec, &|| {
                at.set(at.get() + 1);
                if at.get() >= stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "Vibrance cancellation",
                    ))
                } else {
                    Ok(())
                }
            }));
            assert_eq!(failure.code, ErrorCode::OperationCancelled);
            assert_eq!(pcnd::write(&doc).unwrap(), source);
        }
    }
}
#[test]
fn vibrance_worker7_reuses_parent_budget_and_cumulative_work_without_lut_compilation() {
    use paintcraft_core::types::resources::AllocationClass;
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = document(codec, true);
        let original = pcnd::write(&doc).unwrap();
        let request = packet(&doc, codec);
        let context = match codec {
            Codec::Dds => d::v7::Header::decode(&request[..448]).unwrap().programs,
            Codec::Msp => m::v7::Header::decode(&request[..448]).unwrap().programs,
        };
        assert_eq!(
            context.compile_work(),
            0,
            "Vibrance is direct, not a prepared LUT"
        );
        assert_eq!(
            u32::from_be_bytes(context.encode()[16..20].try_into().unwrap()),
            0
        );
        let budget = SharedBudget::new(64 << 20);
        let admission = WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            64 << 20,
            export::SPATIAL_WORK_MAX,
        );
        admission.reserve_work(7).unwrap();
        with_workspace(admission.clone(), || {
            assert!(matches!(
                execute(&request, codec, &|| Ok(()))?,
                Outcome::Success(..)
            ));
            let first = admission.work_used();
            assert!(first > 7);
            assert!(matches!(
                execute(&request, codec, &|| Ok(()))?,
                Outcome::Success(..)
            ));
            assert_eq!(
                admission.work_used() - first,
                first - 7,
                "same actual request spends same shared counter twice"
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.usage().reserved, 0);
        let empty = SharedBudget::new(0);
        let denied = WorkspaceAdmission::new(
            empty.scope(AllocationClass::SharedBudget),
            64 << 20,
            export::SPATIAL_WORK_MAX,
        );
        assert_eq!(
            context.workspace_peak(),
            0,
            "direct-only chain has no effect workspace"
        );
        with_workspace(denied.clone(), || {
            assert!(matches!(
                execute(&request, codec, &|| Ok(()))?,
                Outcome::Success(..)
            ));
            Ok(())
        })
        .unwrap();
        assert_eq!(empty.usage().reserved, 0);
        // Direct Vibrance legitimately needs zero workspace bytes. A real
        // spatial suffix makes this byte-realm denial causal without inventing
        // allocation requirements for the direct-only control above.
        let mut spatial_doc = doc.clone();
        let layer = spatial_doc.roots[0];
        let mut entries = spatial_doc.layers[&layer].filters.as_slice().to_vec();
        entries.push(FilterInstance::new(
            Uuid::from_u128(53),
            true,
            Descriptor::BoxBlurV1(paintcraft_core::types::filters::BoxBlurV1::new(1).unwrap()),
        ));
        spatial_doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(entries).unwrap();
        let spatial_request = packet(&spatial_doc, codec);
        let spatial_context = match codec {
            Codec::Dds => {
                d::v7::Header::decode(&spatial_request[..448])
                    .unwrap()
                    .programs
            }
            Codec::Msp => {
                m::v7::Header::decode(&spatial_request[..448])
                    .unwrap()
                    .programs
            }
        };
        assert!(spatial_context.workspace_peak() > 0);
        let read_budget = SharedBudget::new(4 << 20);
        let decoded = pcnd::read_with_admission_checked(
            &spatial_request[448..],
            1,
            65_536,
            1,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &read_budget,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &read_budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        let (before_region, after_region, before_programs, after_programs) = match codec {
            Codec::Dds => {
                let h = d::v7::Header::decode(&spatial_request[..448]).unwrap();
                let p = d::SourcePlan::capture_with_policies(
                    &decoded,
                    options(),
                    policies(&decoded, codec),
                )
                .unwrap();
                (
                    h.spatial.encode_programs(),
                    p.spatial_context(spatial_request.len() - 448).unwrap(),
                    h.programs.encode(),
                    p.program_context(spatial_request.len() - 448).unwrap(),
                )
            }
            Codec::Msp => {
                let h = m::v7::Header::decode(&spatial_request[..448]).unwrap();
                let p = m::SourcePlan::capture_with_policies(&decoded, policies(&decoded, codec))
                    .unwrap();
                (
                    h.spatial.encode_programs(),
                    p.spatial_context(spatial_request.len() - 448).unwrap(),
                    h.programs.encode(),
                    p.program_context(spatial_request.len() - 448).unwrap(),
                )
            }
        };
        assert_eq!(before_region, after_region);
        assert_eq!(before_programs, after_programs);
        assert!(
            spatial_doc.layers[&layer].filters.storage_bytes()
                > decoded.layers[&layer].filters.storage_bytes()
        );
        if matches!(codec, Codec::Dds) {
            assert_eq!(spatial_doc.layers[&layer].filters.len(), 2);
            assert_eq!(
                spatial_doc.layers[&layer].filters.storage_bytes(),
                4 * std::mem::size_of::<FilterInstance>()
            );
        }

        let failure = with_workspace(denied, || {
            Ok(error(execute(&spatial_request, codec, &|| Ok(()))))
        })
        .unwrap();
        assert_eq!(failure.code, ErrorCode::ResourceLimit, "{failure:?}");
        assert_eq!(empty.usage().reserved, 0);
        let limited =
            WorkspaceAdmission::new(budget.scope(AllocationClass::SharedBudget), 64 << 20, 7);
        limited.reserve_work(7).unwrap();
        let failure =
            with_workspace(limited, || Ok(error(execute(&request, codec, &|| Ok(()))))).unwrap();
        assert_eq!(failure.code, ErrorCode::ResourceLimit, "{failure:?}");
        assert_eq!(pcnd::write(&doc).unwrap(), original);
    }
}

#[test]
fn generic7_default_tile_elision_keeps_live_and_reopened_resource_contexts_equal() {
    use paintcraft_core::types::Rgba;
    use paintcraft_raster::{Admission, RasterLayer};
    for codec in [Codec::Dds, Codec::Msp] {
        let mut doc = document(codec, true);
        doc.canvas = (128, 128);
        let id = doc.roots[0];
        let mut raster = RasterLayer::new(128, 128, Rgba([255; 4]));
        raster
            .set_block(
                0,
                0,
                &[255u8; 4].repeat(128 * 128),
                &mut Admission::new(65_536),
            )
            .unwrap();
        assert_eq!(
            raster.resident_tiles(),
            1,
            "actual redundant default tile is present"
        );
        doc.layers.get_mut(&id).unwrap().raster = raster;
        let request = packet(&doc, codec);
        let source = &request[448..];
        let budget = SharedBudget::new(4 << 20);
        let reopened = pcnd::read_with_admission_checked(
            source,
            128 * 128,
            65_536,
            1,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(
            reopened.layers[&id].raster.resident_tiles(),
            0,
            "compact writer elides it"
        );
        assert_eq!(pcnd::write_compact(&reopened).unwrap(), source);
        match codec {
            Codec::Dds => {
                let header = d::v7::Header::decode(&request[..448]).unwrap();
                let plan = d::SourcePlan::capture_with_policies(
                    &reopened,
                    options(),
                    policies(&reopened, codec),
                )
                .unwrap();
                assert_eq!(
                    header.spatial.encode_programs(),
                    plan.spatial_context(source.len()).unwrap(),
                    "DDS live/reopened default-tile facts"
                );
                assert_eq!(
                    header.programs.encode(),
                    plan.program_context(source.len()).unwrap()
                );
            }
            Codec::Msp => {
                let header = m::v7::Header::decode(&request[..448]).unwrap();
                let plan =
                    m::SourcePlan::capture_with_policies(&reopened, policies(&reopened, codec))
                        .unwrap();
                assert_eq!(
                    header.spatial.encode_programs(),
                    plan.spatial_context(source.len()).unwrap(),
                    "MSP live/reopened default-tile facts"
                );
                assert_eq!(
                    header.programs.encode(),
                    plan.program_context(source.len()).unwrap()
                );
            }
        }
        assert!(matches!(
            execute(&request, codec, &|| Ok(())).unwrap(),
            Outcome::Success(..)
        ));
        assert_eq!(
            doc.layers[&id].raster.resident_tiles(),
            1,
            "export cannot mutate source"
        );
    }
}

#[test]
fn generic7_explicit_raw_default_record_is_charged_and_underclaim_refuses_before_render() {
    use paintcraft_core::types::Rgba;
    use paintcraft_raster::{Admission, RasterLayer};
    use sha2::{Digest, Sha256};
    fn number(mut value: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        while value >= 128 {
            bytes.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        bytes.push(value as u8);
        bytes
    }
    for codec in [Codec::Dds, Codec::Msp] {
        let mut doc = document(codec, true);
        doc.canvas = (128, 128);
        let id = doc.roots[0];
        let mut raster = RasterLayer::new(128, 128, Rgba([255; 4]));
        raster
            .set_block(
                0,
                0,
                &[255u8; 4].repeat(128 * 128),
                &mut Admission::new(65_536),
            )
            .unwrap();
        doc.layers.get_mut(&id).unwrap().raster = raster;
        let compact = pcnd::write_compact(&doc).unwrap();
        // One layer: its final byte before SHA is the zero tile terminator.
        // Add a valid raw all-default record; the reader accepts AND retains it.
        let end = compact.len() - 33;
        assert_eq!(compact[end], 0);
        let mut raw = compact[..end].to_vec();
        raw.extend_from_slice(&[1, 0, 0]);
        raw.extend(number(128 * 128 * 4));
        raw.extend_from_slice(&[255u8; 4].repeat(128 * 128));
        raw.push(0);
        let inner = Sha256::digest(&raw);
        raw.extend_from_slice(&inner);
        let (witness, source_sha) =
            pcnd::CheckedContainer::verify_with_source_hash(&raw, &|| Ok(())).unwrap();
        assert_eq!(
            witness.snapshot_tile_records_checked(&|| Ok(())).unwrap(),
            1
        );
        assert_eq!(source_sha.as_slice(), Sha256::digest(&raw).as_slice());
        let budget = SharedBudget::new(4 << 20);
        let restored = pcnd::read_with_admission_checked(
            &raw,
            128 * 128,
            65_536,
            1,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(restored.layers[&id].raster.resident_tiles(), 1);
        let mut request = match codec {
            Codec::Dds => {
                let plan =
                    d::SourcePlan::capture_with_policies(&doc, options(), policies(&doc, codec))
                        .unwrap();
                let facts = plan.snapshot_contexts(&raw, &|| Ok(())).unwrap();
                let header =
                    d::v7::Header::capture(&plan, &raw, owner(&doc), doc.document_id).unwrap();
                assert_eq!(header.spatial.encode_programs(), facts.region);
                let decoded_plan = d::SourcePlan::capture_with_policies(
                    &restored,
                    options(),
                    policies(&restored, codec),
                )
                .unwrap();
                assert_eq!(
                    header.spatial.encode_programs(),
                    decoded_plan.spatial_context(raw.len()).unwrap()
                );
                header.encode().to_vec()
            }
            Codec::Msp => {
                let plan =
                    m::SourcePlan::capture_with_policies(&doc, policies(&doc, codec)).unwrap();
                let facts = plan.snapshot_contexts(&raw, &|| Ok(())).unwrap();
                let header =
                    m::v7::Header::capture(&plan, &raw, owner(&doc), doc.document_id).unwrap();
                assert_eq!(header.spatial.encode_programs(), facts.region);
                let decoded_plan =
                    m::SourcePlan::capture_with_policies(&restored, policies(&restored, codec))
                        .unwrap();
                assert_eq!(
                    header.spatial.encode_programs(),
                    decoded_plan.spatial_context(raw.len()).unwrap()
                );
                header.encode().to_vec()
            }
        };
        request.extend_from_slice(&raw);
        assert!(matches!(
            execute(&request, codec, &|| Ok(())).unwrap(),
            Outcome::Success(..)
        ));
        let mut underclaimed = request.clone();
        let payload = u64::from_be_bytes(underclaimed[296..304].try_into().unwrap());
        underclaimed[296..304]
            .copy_from_slice(&(payload - RasterLayer::TILE_BYTES as u64).to_be_bytes());
        let failure = error(execute(&underclaimed, codec, &|| Ok(())));
        assert_eq!(failure.code, ErrorCode::CodecFailed);
        assert_eq!(
            failure.message,
            "Spatial export facts disagree with decoded source"
        );
        assert_eq!(pcnd::write_compact(&doc).unwrap(), compact);
        assert_eq!(doc.layers[&id].raster.resident_tiles(), 1);
    }
}

#[test]
fn snapshot_contexts_bind_actual_source_length_and_reject_extent_before_callbacks() {
    for codec in [Codec::Dds, Codec::Msp] {
        let mut first = document(codec, true);
        first.title = Some("x".into());
        let mut second = first.clone();
        second.title = Some("xy".into());
        let a = pcnd::write_compact(&first).unwrap();
        let b = pcnd::write_compact(&second).unwrap();
        assert_eq!(b.len(), a.len() + 1);
        let capture = |doc: &Document, bytes: &[u8], check: &dyn Fn() -> Result<()>| match codec {
            Codec::Dds => {
                d::SourcePlan::capture_with_policies(doc, options(), policies(doc, codec))
                    .unwrap()
                    .snapshot_contexts(bytes, check)
            }
            Codec::Msp => m::SourcePlan::capture_with_policies(doc, policies(doc, codec))
                .unwrap()
                .snapshot_contexts(bytes, check),
        };
        let before = capture(&first, &a, &|| Ok(())).unwrap();
        let after = capture(&second, &b, &|| Ok(())).unwrap();
        assert_eq!(
            u64::from_be_bytes(after.region[40..48].try_into().unwrap())
                - u64::from_be_bytes(before.region[40..48].try_into().unwrap()),
            1
        );
        assert_eq!(before.programs, after.programs);
        assert_ne!(before.source_sha256, after.source_sha256);
        assert_eq!(
            capture(&first, &[], &|| panic!("empty extent called back"))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        let cap = match codec {
            Codec::Dds => d::SOURCE_BYTES_MAX,
            Codec::Msp => m::SOURCE_BYTES_MAX,
        };
        assert_eq!(
            capture(&first, &vec![0; cap + 1], &|| panic!(
                "oversized extent called back"
            ))
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(
            capture(&first, &a, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "snapshot cancellation"
            )))
            .unwrap_err()
            .code,
            ErrorCode::OperationCancelled
        );
    }
}
