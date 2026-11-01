//! Bounded bootstrap only. No metadata query, authenticated fact or payload here.
use super::{capture::Capture7, echo};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
};
use paintcraft_formats::export::layer_appearance::Mode7;

pub(in crate::app::filter_editing) fn encode(source: &Capture7) -> Result<Value> {
    let value = Value::object([
        ("sourceProtocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        (
            "mode",
            Value::str(match source.admission.mode {
                Mode7::Base => "base",
                Mode7::Noise => "noise",
            }),
        ),
        ("owner", echo::owner(source)),
        ("draft", Value::str(source.draft.to_string())),
    ]);
    // Construction is closed by type. UTF-8 bound is enforced before bridge work.
    if json::canonical(&value).len() > 8192 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Source7 bootstrap UTF-8 byte bound",
        ));
    }
    Ok(value)
}
