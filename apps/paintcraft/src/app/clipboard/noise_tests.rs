use super::*;
use paintcraft_core::types::{
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
    ids::deterministic_id,
};
use paintcraft_document::{FilterInstance, FilterStack, POLICY};
use std::{sync::Mutex, time::Instant};
#[derive(Clone, Default)]
struct Fake {
    images: Arc<Mutex<Vec<Image>>>,
    ack: Arc<Mutex<Option<std::result::Result<(), String>>>>,
}
impl WriteJob for Fake {
    fn poll(&self) -> Option<std::result::Result<(), String>> {
        self.ack.lock().unwrap().take()
    }
}
impl Writer for Fake {
    fn submit(
        &self,
        image: Image,
        _: &egui::Context,
    ) -> std::result::Result<Box<dyn WriteJob>, String> {
        self.images.lock().unwrap().push(image);
        Ok(Box::new(self.clone()))
    }
}
#[test]
fn spawned_visible_noise_copy_keeps_parent_until_ack_and_raw_target_copy_remains_unfiltered() {
    for (distribution, rendered) in [
        (
            NoiseDistribution::Uniform,
            [255, 211, 0, 255, 51, 0, 97, 255],
        ),
        (
            NoiseDistribution::Gaussian,
            [47, 139, 0, 255, 0, 4, 46, 255],
        ),
    ] {
        let mut doc =
            paintcraft_document::new_document(2, 1, Some(Rgba([64, 16, 128, 255])), 0, POLICY)
                .unwrap();
        let id = doc.roots[0];
        doc.layers.get_mut(&id).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["visible clipboard Noise"]),
            true,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(100, 65535, false, distribution).unwrap()),
        )])
        .unwrap();
        let source = Arc::new(doc);
        let before = source.clone();
        let ctx = egui::Context::default();
        for visible in [true, false] {
            let fake = Fake::default();
            let mut state = State {
                writer: Some(Box::new(fake.clone())),
                ..Default::default()
            };
            let parent = SharedBudget::new(128 << 20);
            let owner = deterministic_id(&["clipboard owner"]);
            assert!(
                state
                    .start(
                        owner,
                        source.clone(),
                        Selection::Inactive,
                        if visible { None } else { Some(id) },
                        None,
                        &ctx,
                        &parent
                    )
                    .unwrap()
            );
            let deadline = Instant::now() + Duration::from_secs(10);
            while !matches!(
                state.pending.as_ref().map(|p| &p.stage),
                Some(Stage::Write { .. })
            ) {
                assert!(state.poll(&ctx).is_none());
                assert!(Instant::now() < deadline, "Noise clipboard capture stalled");
                std::thread::yield_now();
            }
            assert!(state.last_capture.is_none());
            let expected = if visible {
                rendered
            } else {
                [64, 16, 128, 255, 64, 16, 128, 255]
            };
            assert_eq!(fake.images.lock().unwrap()[0].rgba(), expected);
            assert_eq!(
                parent.usage().reserved,
                if visible { spatial_workspace::BYTES } else { 0 }
            );
            assert!(source.same_semantics(&before));
            *fake.ack.lock().unwrap() = Some(Ok(()));
            assert_eq!(state.poll(&ctx), Some((owner, Ok(true), None)));
            assert_eq!(parent.usage().reserved, 0);
            assert_eq!(state.last_capture.as_ref().unwrap().image.rgba(), expected);
        }
    }
}
