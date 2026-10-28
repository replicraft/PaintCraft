//! Pure prototypes only: no source intake, GUI dispatch, or publication authority.
//! A caller supplies a complete Gate-drained segment. Adjacent ordinals do not
//! establish native callback identity. Runtime adoption remains unsupported.
#![allow(dead_code)]
use paintcraft_platform::tablet::{ImeComponent, NativeUi, WindowsCharacterKind};

mod decode;
mod targets;
#[cfg(test)]
mod tests;

const LIMIT: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    Capacity,
    Order,
    Text,
    Composition,
    Unsupported,
    Target,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Observation {
    installation: u64,
    ordinal: u64,
    fact: NativeUi,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Evidence {
    first: Observation,
    last: Observation,
}
#[derive(Debug, PartialEq, Eq)]
enum Value {
    Fact(NativeUi),
    Text {
        scalar: char,
        repeat: u16,
        system: bool,
    },
    Component {
        composition: u64,
        kind: ImeComponent,
        values: Vec<u32>,
        text: Option<String>,
        cursor_native: Option<u32>,
        native_flags: u32,
    },
}
#[derive(Debug, PartialEq, Eq)]
struct Decoded {
    evidence: Evidence,
    value: Value,
}
fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), Fault> {
    values.try_reserve_exact(count).map_err(|_| Fault::Capacity)
}
