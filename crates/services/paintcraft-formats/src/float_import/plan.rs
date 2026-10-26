//! Pure source ownership and reconstruction of a bounded numeric worker report.
use super::*;
pub const INPUT_BYTES_MAX: usize = 16 << 20;
pub const PIXELS_MAX: u64 = 1_048_576;
pub(super) fn allowance(bytes: &[u8], pixels: u64) -> Result<()> {
    if bytes.is_empty() || bytes.len() > INPUT_BYTES_MAX || pixels == 0 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Private float worker source allowance",
        ));
    }
    Ok(())
}
#[derive(Debug)]
pub struct SourcePlan {
    pub(super) binding: Binding,
    pub(super) source: Source,
    pub(super) base_losses: Vec<&'static str>,
    pub(super) _metadata: BudgetAccount,
    pub(super) budget: SharedBudget,
}
#[derive(Debug)]
pub struct WorkerAssessment {
    pub(super) assessment: Assessment,
    pub(super) _metadata: BudgetAccount,
}
impl WorkerAssessment {
    pub fn assessment(&self) -> &Assessment {
        &self.assessment
    }
}
impl SourcePlan {
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub(super) fn retains_alpha(&self) -> bool {
        self.binding.settings().alpha_output == precision::AlphaOutput::Preserve
            && self.source.channels.iter().any(|(name, _)| *name == "A")
    }
    /// RGBA remains caller-admitted. This verifies a report, not source file
    /// freshness/editor ownership, nor independent original-float statistics.
    pub fn accept_report(
        self,
        report: wire::Report,
        size: (u32, u32),
        rgba: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<WorkerAssessment> {
        let _scratch = scratch(&self.budget)?;
        if size != self.binding.size() {
            return Err(invalid("Float worker dimensions changed"));
        }
        let result = self.assessment(report.statistics, report.partial, rgba, check)?;
        if result.assessment.digest() != report.digest {
            return Err(invalid(
                "Float worker assessment/source/settings/output digest mismatch",
            ));
        }
        Ok(result)
    }
    pub(super) fn assessment(
        mut self,
        statistics: precision::Statistics,
        partial: u64,
        rgba: &[u8],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<WorkerAssessment> {
        check()?;
        let (w, h) = self.binding.size();
        let pixels = u64::from(w) * u64::from(h);
        if rgba.len() as u64 != pixels * 4 {
            return Err(invalid("Float worker RGBA extent mismatch"));
        }
        if statistics.negative_source_samples > pixels * 3
            || statistics.negative_target_samples_clipped > pixels * 3
            || statistics.highlights_clipped > pixels * 3
            || statistics.additive_zero_alpha_pixels > pixels
            || statistics.alpha_samples_quantized > pixels
            || partial > pixels
        {
            return Err(invalid("Float worker statistic allowance exceeded"));
        }
        let settings = self.binding.settings();
        let retains = self.retains_alpha();
        if (!retains && (partial != 0 || statistics.alpha_samples_quantized != 0))
            || (settings.curve != precision::Curve::Clip && statistics.highlights_clipped != 0)
            || (settings.negative == precision::Negative::Reject
                && (statistics.negative_source_samples != 0
                    || statistics.negative_target_samples_clipped != 0))
            || ((settings.alpha_output == precision::AlphaOutput::Preserve
                || self.binding.effective_alpha() != precision::SourceAlpha::Premultiplied)
                && statistics.additive_zero_alpha_pixels != 0)
            || (retains && statistics.alpha_samples_quantized > partial)
        {
            return Err(invalid(
                "Float worker statistics contradict selected policy",
            ));
        }
        let mut output_partial = 0;
        let mut hash = Sha256::new();
        for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            if index % 1024 == 0 {
                check()?;
            }
            if pixel[3] > 0 && pixel[3] < 255 {
                output_partial += 1;
            }
            if !retains && pixel[3] != 255 {
                return Err(invalid("Float worker opaque policy returned transparency"));
            }
        }
        if output_partial > partial {
            return Err(invalid(
                "Float worker retained source partial-alpha count is inconsistent",
            ));
        }
        for chunk in rgba.chunks(64 << 10) {
            check()?;
            hash.update(chunk);
        }
        for loss in precision::classify_losses(&self.binding, statistics) {
            if !self.base_losses.contains(&loss) {
                self.base_losses.push(loss);
            }
        }
        if partial > 0 {
            self.base_losses
                .push("hdr-linear-alpha-compositing-changed");
        }
        let mut assessment = Assessment {
            binding: self.binding,
            source: self.source,
            losses: self.base_losses,
            rgba_sha256: hash.finalize().into(),
            digest: [0; 32],
            statistics,
            retained_partial_alpha_pixels: partial,
        };
        let mut hash = Sha256::new();
        hash.update(b"PaintCraft float import assessment v1\0");
        hash.update(json::canonical(&assessment.to_value()));
        assessment.digest = hash.finalize().into();
        check()?;
        Ok(WorkerAssessment {
            assessment,
            _metadata: self._metadata,
        })
    }
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
