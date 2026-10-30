use super::*;

#[derive(clap::Args, Debug)]
pub struct Options {
    #[arg(value_parser = ["batch", "batch-v2", "batch-v3", "result-v2", "result-v3", "file-command-v2", "file-command-v3", "inspect-v2", "mcp-inspect-v2", "palette-inspection", "shape-inspection", "brush-inspection", "source-inspection"])]
    schema: String,
}

pub fn run(a: Options, _json: bool) -> Reply {
    // The schema is the output; it is always JSON, with or without --json.
    bare(
        true,
        (|| {
            if a.schema == "source-inspection" {
                return Ok((inspect_source::schema()?, String::new()));
            }
            if a.schema == "brush-inspection" {
                return Ok((brushes::schema()?, String::new()));
            }
            if a.schema == "shape-inspection" {
                return Ok((shapes::schema()?, String::new()));
            }
            let text = match a.schema.as_str() {
                "batch" => BATCH_SCHEMA,
                "batch-v2" => BATCH_V2_SCHEMA,
                "batch-v3" => BATCH_V3_SCHEMA,
                "result-v3" => include_str!("../../schemas/result-v3.schema.json"),
                "file-command-v3" => include_str!("../../schemas/file-command-v3.schema.json"),
                "inspect-v2" => include_str!("../../schemas/inspect-v2.schema.json"),
                "mcp-inspect-v2" => include_str!("../../schemas/mcp-inspect-v2.schema.json"),
                "result-v2" => include_str!("../../schemas/result-v2.schema.json"),
                "file-command-v2" => include_str!("../../schemas/file-command-v2.schema.json"),
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
