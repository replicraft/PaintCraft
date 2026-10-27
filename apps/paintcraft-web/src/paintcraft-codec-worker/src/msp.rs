//! Browser-only MSP2 entry; the default MSP1 and decoder2 paths are independent.
use paintcraft_core::types::Result;
use paintcraft_formats::msp::web_export;
pub fn execute(bytes: &[u8]) -> Result<Vec<u8>> {
    web_export::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_formats::codec_worker::Owner;
    #[test]
    fn actual_msp2_entry_uses_bound_preview_and_keeps_other_protocols_separate() {
        let doc = paintcraft_document::new_document(
            9,
            3,
            Some(paintcraft_core::types::Rgba([255, 255, 255, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let plan = web_export::SourcePlan::capture(&doc).unwrap();
        let source = plan.snapshot(&doc).unwrap();
        let owner = Owner {
            session: doc.document_id,
            state: doc.state_id,
            epoch: 0,
            frontend: 3,
        };
        let header = web_export::Header::capture(&plan, &source, owner, doc.document_id).unwrap();
        let mut request = header.encode().to_vec();
        request.extend(source);
        let response = execute(&request).unwrap();
        match web_export::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION"))
            .unwrap()
        {
            web_export::Frame::Success {
                preview, encoded, ..
            } => {
                assert_eq!(preview, [255; 9 * 3 * 4]);
                assert_eq!(&encoded[..4], b"LinS");
            }
            web_export::Frame::Refused(error) => panic!("{error:?}"),
        }
        for magic in [b"PCDDSQ01", b"PCDEC002"] {
            request[..8].copy_from_slice(magic);
            assert!(execute(&request).is_err());
        }
    }
}
