//! Exact metadata publication; endpoint parsing/adoption is separate.
use super::*;
use paintcraft_core::types::{json::Value, policy::hex};
fn number(n: u64) -> Value {
    Value::str(n.to_string())
}
impl BlendRegistration {
    pub fn value(self) -> Value {
        Value::object([
            ("feature_id", Value::str(self.feature_id)),
            (
                "resource_contract_id",
                Value::str(self.resource_contract_id),
            ),
            (
                "resource_contract_revision",
                Value::Int(i64::from(self.resource_contract_revision)),
            ),
            (
                "arithmetic_revision",
                Value::Int(i64::from(self.arithmetic_revision)),
            ),
            ("work_revision", Value::Int(i64::from(self.work_revision))),
            (
                "context_setup_per_root",
                number(self.context_setup_per_root),
            ),
            ("opaque_extra_work", number(self.opaque_extra_work)),
            ("general_work", number(self.general_work)),
            ("workload_class_id", Value::str(self.workload_class_id)),
            (
                "workload_class_revision",
                Value::Int(i64::from(self.workload_class_revision)),
            ),
        ])
    }
}
impl Admission8 {
    pub fn value(self) -> Result<Value> {
        self.validate()?;
        Ok(Value::object([
            (
                "mode",
                Value::str(match self.mode {
                    Mode7::Base => "base",
                    Mode7::Noise => "noise",
                }),
            ),
            ("workAuthority", Value::str(self.work_authority.id())),
            ("workspaceBytes", number(self.workspace_bytes)),
            ("baseWorkLimit", number(self.base_work)),
            ("noiseWorkLimit", number(self.noise_work)),
            ("totalWorkLimit", number(self.total()?)),
        ]))
    }
    pub fn slice_value(self) -> Value {
        Value::object([
            ("revision", Value::Int(1)),
            ("nativeEdge", Value::Int(i64::from(Self::EDGE))),
            ("baseWorkLimit", number(Self::BASE_SLICE)),
            ("noiseWorkLimit", number(self.noise_slice())),
        ])
    }
}
impl Fact8 {
    pub fn value(self) -> Value {
        Value::object([
            ("count", Value::Int(self.count as i64)),
            ("requiredSha256", Value::str(hex(&self.required))),
            ("resolutionSha256", Value::str(hex(&self.resolution))),
            ("layerMasks", self.own.value()),
            (
                "sourceCoverageSha256",
                Value::str(hex(&self.source_coverage)),
            ),
            ("maskInventorySha256", Value::str(hex(&self.mask_inventory))),
            ("layerBlendSha256", Value::str(hex(&self.layer_blend))),
            ("capabilitySha256", Value::str(hex(&self.capability))),
        ])
    }
}
impl Union8 {
    pub fn value(self) -> Value {
        Value::object([
            ("count", Value::Int(self.count as i64)),
            ("requiredSha256", Value::str(hex(&self.required))),
            ("resolutionSha256", Value::str(hex(&self.resolution))),
            (
                "sourceCoverageSha256",
                Value::str(hex(&self.source_coverage)),
            ),
            ("layerBlendSha256", Value::str(hex(&self.layer_blend))),
            ("capabilitySha256", Value::str(hex(&self.capability))),
        ])
    }
}
