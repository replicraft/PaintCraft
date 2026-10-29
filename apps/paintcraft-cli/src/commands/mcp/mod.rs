//! Ephemeral headless painting session. Stdout belongs exclusively to MCP.
mod brushes;
mod dcx;
mod dds;
mod floats;
mod icns;
mod icons;
mod jxr;
mod layered;
mod msp;
mod native_codec;
mod palettes;
mod params;
mod shapes;
mod source_inspection;
mod state;
mod tools;

use rmcp::ServiceExt;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock};
use state::State;
use std::sync::{Arc, Mutex};

#[derive(Debug, clap::Args)]
pub struct Options {}

#[derive(Clone)]
pub struct PaintingServer {
    state: Arc<Mutex<State>>,
    tool_router: ToolRouter<Self>,
}

impl PaintingServer {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            tool_router: Self::router(),
        }
    }

    async fn work(
        &self,
        job: impl FnOnce(&mut State) -> paintcraft_core::types::Result<CallToolResult> + Send + 'static,
    ) -> CallToolResult {
        let state = self.state.clone();
        match tokio::task::spawn_blocking(move || {
            let mut state = state
                .lock()
                .map_err(|_| state::invalid("painting session unavailable"))?;
            job(&mut state)
        })
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => CallToolResult::error(vec![ContentBlock::text(
                String::from_utf8_lossy(&crate::report::json_line(&crate::report::error_value(
                    &error,
                )))
                .into_owned(),
            )]),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(format!(
                "painting worker failed: {error}"
            ))]),
        }
    }
}

pub fn run(_: Options) -> std::process::ExitCode {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?
            .block_on(async {
                let server = PaintingServer::new()
                    .serve(rmcp::transport::stdio())
                    .await?;
                server.waiting().await?;
                Ok(())
            })
    })();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("MCP server: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
