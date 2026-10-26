use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    #[arg(value_parser = ["batch", "batch-v2", "palette-inspection", "shape-inspection"])]
    schema: String,
}

pub fn run(a: Options, _json: bool) -> Reply {
    // The schema is the output; it is always JSON, with or without --json.
    bare(
        true,
        (|| {
            if a.schema == "shape-inspection" {
                return Ok((shapes::schema()?, String::new()));
            }
            let text = match a.schema.as_str() {
                "batch" => BATCH_SCHEMA,
                "batch-v2" => BATCH_V2_SCHEMA,
                "palette-inspection" => {
                    include_str!("../../schemas/palette-inspection.schema.json")
                }
                _ => {
                    return Err(Error::new(ErrorCode::InputSchema, "unknown CLI schema"));
                }
            };
            Ok((
                paintcraft_core::types::json::parse(text.as_bytes())
                    .expect("embedded schema is valid JSON"),
                String::new(),
            ))
        })(),
    )
}
