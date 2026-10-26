use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleType {
    Half,
    Float,
}
impl SampleType {
    pub fn bytes(self) -> u32 {
        match self {
            Self::Half => 2,
            Self::Float => 4,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel<'a> {
    pub name: &'a str,
    pub sample_type: SampleType,
    pub perceptually_linear: bool,
}
pub(super) fn parse<'a>(data: &'a [u8]) -> Result<Vec<Channel<'a>>> {
    let mut bytes = Bytes { bytes: data, at: 0 };
    let mut out: Vec<Channel<'a>> = Vec::new();
    loop {
        let name = bytes.text()?;
        if name.is_empty() {
            break;
        }
        if !matches!(name, "R" | "G" | "B" | "A") || out.len() == 4 {
            return Err(unsupported(
                "EXR requires exactly top-level RGB or RGBA channels",
            ));
        }
        if out.iter().any(|c| c.name == name) {
            return Err(invalid("Duplicate EXR channel"));
        }
        if out.last().is_some_and(|c| c.name >= name) {
            return Err(invalid("EXR channel list is not sorted"));
        }
        let sample_type = match bytes.u32()? {
            1 => SampleType::Half,
            2 => SampleType::Float,
            _ => return Err(unsupported("EXR UINT/unknown channel type unqualified")),
        };
        let flags = bytes.take(4)?;
        if flags[0] > 1 || flags[1..] != [0, 0, 0] {
            return Err(invalid("EXR channel flags/reserved bytes invalid"));
        }
        if bytes.i32()? != 1 || bytes.i32()? != 1 {
            return Err(unsupported("EXR subsampled channels unqualified"));
        }
        out.push(Channel {
            name,
            sample_type,
            perceptually_linear: flags[0] == 1,
        });
    }
    if bytes.at != data.len()
        || !["R", "G", "B"]
            .iter()
            .all(|n| out.iter().any(|c| c.name == *n))
    {
        return Err(unsupported(
            "EXR channel list must contain complete RGB, optional A only",
        ));
    }
    Ok(out)
}
