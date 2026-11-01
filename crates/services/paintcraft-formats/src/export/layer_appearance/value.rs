//! Bounded metadata publication only; strict acceptance belongs endpoint parser.
use super::*;
use paintcraft_core::types::{json::Value, policy::hex};
fn number(n: u64) -> Value {
    Value::str(n.to_string())
}
impl AppearanceRegistration {
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
            ("query_revision", Value::Int(i64::from(self.query_revision))),
            ("alpha_revision", Value::Int(i64::from(self.alpha_revision))),
            ("workload_class_id", Value::str(self.workload_class_id)),
            (
                "workload_class_revision",
                Value::Int(i64::from(self.workload_class_revision)),
            ),
        ])
    }
}
impl Admission7 {
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
impl LayerMasks7 {
    pub fn value(self) -> Value {
        Value::object([
            ("stored", Value::Int(i64::from(self.stored))),
            ("enabled", Value::Int(i64::from(self.enabled))),
            ("bands", Value::Int(self.bands as i64)),
            ("runs", Value::Int(self.runs as i64)),
            ("decodeUpperBytes", number(self.decode_upper_bytes)),
            ("identitySha256", Value::str(hex(&self.identity))),
            ("coverageQuoteRevision", Value::Int(1)),
            ("coverageAbi", Value::str(self.abi.id())),
        ])
    }
}
impl Fact7 {
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
            ("capabilitySha256", Value::str(hex(&self.capability))),
        ])
    }
}
impl Union7 {
    pub fn value(self) -> Value {
        Value::object([
            ("count", Value::Int(self.count as i64)),
            ("requiredSha256", Value::str(hex(&self.required))),
            ("resolutionSha256", Value::str(hex(&self.resolution))),
            (
                "sourceCoverageSha256",
                Value::str(hex(&self.source_coverage)),
            ),
            ("capabilitySha256", Value::str(hex(&self.capability))),
        ])
    }
}
