use super::*;

/// Admission estimates for renderer intermediates and work, not a hard allocator
/// or wall-clock limit. Parser/font/codec allocations remain upstream-owned.
pub(super) fn admit(
    svg: &usvg::Tree,
    transform: tiny_skia::Transform,
    options: Options,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let mut budget = Budget {
        nodes: 0,
        scratch: u64::from(options.width) * u64::from(options.height) * 4,
        work: 0,
        transform,
        options,
        check,
    };
    budget.group(svg.root(), 0)
}
struct Budget<'a> {
    nodes: u32,
    scratch: u64,
    work: u64,
    transform: tiny_skia::Transform,
    options: Options,
    check: &'a dyn Fn() -> Result<()>,
}
impl Budget<'_> {
    fn group(&mut self, group: &usvg::Group, depth: usize) -> Result<()> {
        (self.check)()?;
        if depth > DEPTH_MAX {
            return Err(resource("SVG prepared subtree nesting exceeds allowance"));
        }
        let pixels = self.area(group)?;
        if group.should_isolate() {
            self.charge(
                pixels
                    * 4
                    * (8 + group
                        .filters()
                        .iter()
                        .map(|f| f.primitives().len() as u64 * 4)
                        .sum::<u64>()),
            )?;
        }
        let transform = self.transform.pre_concat(group.abs_transform());
        let scale = (transform.sx.hypot(transform.ky)).max(transform.kx.hypot(transform.sy));
        let mut primitives = 0usize;
        for filter in group.filters() {
            for primitive in filter.primitives() {
                (self.check)()?;
                primitives += 1;
                if primitives > 64 {
                    return Err(resource("SVG filter primitive count exceeds allowance"));
                }
                use usvg::filter::Kind;
                let factor = match primitive.kind() {
                    Kind::Morphology(f) => {
                        let radius = f.radius_x().get().max(f.radius_y().get()) * scale;
                        if !radius.is_finite() || radius > 64.0 {
                            return Err(resource("SVG scaled morphology radius exceeds allowance"));
                        }
                        (radius.ceil() as u64 * 2 + 1).pow(2)
                    }
                    Kind::Turbulence(f) => u64::from(f.num_octaves()) * 32,
                    Kind::ConvolveMatrix(f) => {
                        u64::from(f.matrix().columns()) * u64::from(f.matrix().rows())
                    }
                    Kind::GaussianBlur(f) => {
                        self.blur(f.std_dev_x().get().max(f.std_dev_y().get()) * scale)?;
                        16
                    }
                    Kind::DropShadow(f) => {
                        self.blur(f.std_dev_x().get().max(f.std_dev_y().get()) * scale)?;
                        20
                    }
                    _ => 8,
                };
                self.work = self
                    .work
                    .checked_add(pixels.saturating_mul(factor))
                    .filter(|n| *n <= 128_000_000)
                    .ok_or_else(|| resource("SVG estimated filter work exceeds allowance"))?;
            }
        }
        for node in group.children() {
            (self.check)()?;
            self.nodes += 1;
            if self.nodes > NODES_MAX {
                return Err(resource("SVG prepared node count exceeds allowance"));
            }
            if let usvg::Node::Group(child) = node {
                self.group(child, depth + 1)?;
            }
            let mut result = Ok(());
            node.subroots(|root| {
                if result.is_ok() {
                    result = self.group(root, depth + 1);
                }
            });
            result?;
        }
        Ok(())
    }
    fn area(&self, group: &usvg::Group) -> Result<u64> {
        let bounds = group
            .abs_layer_bounding_box()
            .transform(self.transform)
            .ok_or_else(|| resource("SVG transformed bounds exceed allowance"))?;
        let width =
            (f64::from(bounds.width()).ceil() + 4.0).min(f64::from(self.options.width) * 5.0);
        let height =
            (f64::from(bounds.height()).ceil() + 4.0).min(f64::from(self.options.height) * 5.0);
        if !width.is_finite() || !height.is_finite() {
            return Err(resource("SVG intermediate bounds are not finite"));
        }
        Ok(width as u64 * height as u64)
    }
    fn charge(&mut self, bytes: u64) -> Result<()> {
        self.scratch = self
            .scratch
            .checked_add(bytes)
            .filter(|n| *n <= 128 << 20)
            .ok_or_else(|| resource("SVG estimated render scratch exceeds allowance"))?;
        Ok(())
    }
    fn blur(&self, deviation: f32) -> Result<()> {
        if !deviation.is_finite() || deviation > 256.0 {
            return Err(resource("SVG scaled blur deviation exceeds allowance"));
        }
        Ok(())
    }
}
