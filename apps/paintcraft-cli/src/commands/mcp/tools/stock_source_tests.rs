//! Actual rmcp requests with an exact-path, one-shot final-adoption mutation.
use super::super::PaintingServer;
use paintcraft_formats::image::{self, ImageFormat};
use rmcp::ServiceExt;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
static MUTATIONS: Mutex<Vec<(PathBuf, bool)>> = Mutex::new(Vec::new());
pub(super) fn before_adoption(path: &Path) {
    let mutation = {
        let mut entries = MUTATIONS.lock().unwrap();
        entries
            .iter()
            .position(|(p, _)| p == path)
            .map(|index| entries.remove(index).1)
    };
    if let Some(replace) = mutation {
        let bytes = fs::read(path).unwrap();
        if replace {
            fs::rename(path, path.with_extension("original")).unwrap();
            fs::write(path, bytes).unwrap();
        } else {
            let mut changed = bytes;
            *changed.last_mut().unwrap() ^= 1;
            fs::write(path, changed).unwrap();
        }
    }
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/stock-source/duplex")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        MUTATIONS
            .lock()
            .unwrap()
            .retain(|(p, _)| !p.starts_with(&self.0));
        fs::remove_dir_all(&self.0).unwrap();
    }
}
async fn response(
    reader: &mut BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    id: u64,
) -> Value {
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).await.unwrap() > 0);
        let value: Value = serde_json::from_str(&line).unwrap();
        if value["id"] == id {
            return value["result"].clone();
        }
    }
}
async fn send(writer: &mut tokio::io::WriteHalf<tokio::io::DuplexStream>, value: Value) {
    writer
        .write_all(format!("{value}\n").as_bytes())
        .await
        .unwrap();
}
async fn call(
    reader: &mut BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>,
    writer: &mut tokio::io::WriteHalf<tokio::io::DuplexStream>,
    id: u64,
    name: &str,
    args: Value,
) -> Value {
    send(writer,json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await;
    response(reader, id).await
}
fn body(result: &Value) -> Value {
    assert_ne!(result["isError"], true, "{result}");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}
#[test]
fn real_rmcp_changed_source_refusal_preserves_prior_dirty_session_and_history() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
        for replace in [false,true] {
            let fixture=Fixture::new();let path=fixture.0.join("source.png");let bytes=image::encode(ImageFormat::Png,2,1,&[13,71,201,255,7,8,9,255],90).unwrap();fs::write(&path,&bytes).unwrap();
            let (client,server)=tokio::io::duplex(65536);let server_task=tokio::spawn(async move {PaintingServer::new().serve(server).await.unwrap().waiting().await.unwrap()});
            let (read,mut write)=tokio::io::split(client);let mut read=BufReader::new(read);
            send(&mut write,json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"stock-source-proof","version":"1"}}})).await;response(&mut read,0).await;
            send(&mut write,json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
            body(&call(&mut read,&mut write,1,"create_canvas",json!({"width":3,"height":2})).await);
            body(&call(&mut read,&mut write,2,"paint_stroke",json!({"points":[{"x":0,"y":0},{"x":2,"y":1}],"color":"#123456FF","size":1,"brush":"round"})).await);
            let before=body(&call(&mut read,&mut write,3,"inspect_document",json!({})).await);assert_eq!(before["dirty"],true);
            MUTATIONS.lock().unwrap().push((path.clone(),replace));
            let refused=call(&mut read,&mut write,4,"import_image",json!({"path":path,"discard_changes":true})).await;assert_eq!(refused["isError"],true,"{refused}");
            let error:Value=serde_json::from_str(refused["content"][0]["text"].as_str().unwrap()).unwrap();assert_eq!(error["schema"],"paintcraft.error/v1");assert_eq!(error["code"],"conflict.stale_base");assert_eq!(error["committed"],false);
            assert_eq!(body(&call(&mut read,&mut write,5,"inspect_document",json!({})).await),before);
            body(&call(&mut read,&mut write,6,"history",json!({})).await);let undone=body(&call(&mut read,&mut write,7,"inspect_document",json!({})).await);assert_ne!(undone["state_id"],before["state_id"]);
            body(&call(&mut read,&mut write,8,"history",json!({"redo":true})).await);assert_eq!(body(&call(&mut read,&mut write,9,"inspect_document",json!({})).await),before);
            assert!(!MUTATIONS.lock().unwrap().iter().any(|(p,_)|p==&path));
            if replace {assert_eq!(fs::read(path.with_extension("original")).unwrap(),bytes);}
            drop(read);drop(write);server_task.await.unwrap();
        }
        }).await.expect("bounded real rmcp proof");
    });
}
