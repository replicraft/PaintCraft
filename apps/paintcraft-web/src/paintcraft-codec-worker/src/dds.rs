//! Browser-specific DDS entry; it neither changes nor falls back to decoder2.
use paintcraft_core::types::Result;
use paintcraft_formats::dds::web_export;
pub fn execute(bytes: &[u8]) -> Result<Vec<u8>> {
    web_export::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_formats::{
        codec_worker::Owner,
        dds::{
            Encoding, MipCount, MipFilter, SourceAlpha, TextureOptions, TextureQuality, Transfer,
        },
    };
    #[test]
    fn actual_worker_entry_completes_linear_and_keeps_decoder2_separate() {
        let doc = paintcraft_document::new_document(
            1,
            1,
            Some(paintcraft_core::types::Rgba([128, 188, 255, 127])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let options = TextureOptions {
            encoding: Encoding::Rgba8,
            transfer: Transfer::Linear,
            alpha: SourceAlpha::Straight,
            mips: MipCount::Levels(1),
            filter: MipFilter::Nearest,
            quality: TextureQuality::Normal,
        };
        let plan = web_export::SourcePlan::capture(&doc, options).unwrap();
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
            web_export::Frame::Success { preview, .. } => assert_eq!(preview, [55, 128, 255, 127]),
            web_export::Frame::Refused(error) => panic!("{error:?}"),
        }
        request[..8].copy_from_slice(b"PCDEC002");
        assert!(execute(&request).is_err());
    }
}
