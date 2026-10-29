//! Actual rmcp cancellation/freshness with an isolated one-shot final-check gate.
use super::super::PaintingServer;
use crate::commands::inspect_source::tests::{remove_under, schedule};
use rmcp::ServiceExt;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/validation/source-inspection/duplex")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        remove_under(&self.0);
        fs::remove_dir_all(&self.0).unwrap();
    }
}
async fn send(writer: &mut tokio::io::WriteHalf<tokio::io::DuplexStream>, value: Value) {
    writer
        .write_all(format!("{value}\n").as_bytes())
        .await
        .unwrap();
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
fn body(value: Value) -> Value {
    assert_ne!(value["isError"], true, "{value}");
    serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap()
}
#[test]
fn real_rmcp_final_source_replacement_and_cancel_preserve_dirty_session_and_undo() {
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap().block_on(async{tokio::time::timeout(Duration::from_secs(30),async{
 for mode in 0..3{let folder=Folder::new();let path=folder.0.join("input.bmp");let bytes=include_bytes!("../../../../../../crates/services/paintcraft-formats/tests/bmp-rle-fixtures/rle8-padding.bmp");fs::write(&path,bytes).unwrap();let(client,server)=tokio::io::duplex(65536);let server=tokio::spawn(async move{PaintingServer::new().serve(server).await.unwrap().waiting().await.unwrap()});let(read,mut write)=tokio::io::split(client);let mut read=BufReader::new(read);
 send(&mut write,json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"inspection-proof","version":"1"}}})).await;response(&mut read,0).await;send(&mut write,json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
 body(call(&mut read,&mut write,1,"create_canvas",json!({"width":3,"height":2})).await);body(call(&mut read,&mut write,2,"paint_stroke",json!({"points":[{"x":0,"y":0},{"x":2,"y":1}],"color":"#123456FF","size":1,"brush":"round"})).await);let before=body(call(&mut read,&mut write,3,"inspect_document",json!({})).await);assert_eq!(before["dirty"],true);
 if mode<2{let input=path.clone();schedule(path.clone(),move||{if mode==0{let mut b=fs::read(&input).unwrap();*b.last_mut().unwrap()^=1;fs::write(input,b).unwrap();}else{let b=fs::read(&input).unwrap();fs::rename(&input,input.with_extension("original")).unwrap();fs::write(input,b).unwrap();}});let refused=call(&mut read,&mut write,4,"inspect_source",json!({"path":path})).await;assert_eq!(refused["isError"],true);let error:Value=serde_json::from_str(refused["content"][0]["text"].as_str().unwrap()).unwrap();assert_eq!(error["code"],"conflict.stale_base");assert_eq!(error["committed"],false);
 }else{let(ready_tx,ready_rx)=tokio::sync::oneshot::channel();let(resume_tx,resume_rx)=std::sync::mpsc::channel();schedule(path.clone(),move||{ready_tx.send(()).unwrap();resume_rx.recv_timeout(Duration::from_secs(10)).unwrap();});send(&mut write,json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"inspect_source","arguments":{"path":path}}})).await;ready_rx.await.unwrap();send(&mut write,json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":4,"reason":"inspection cancel control"}})).await;send(&mut write,json!({"jsonrpc":"2.0","id":40,"method":"ping"})).await;
                response(&mut read,40).await;
                resume_tx.send(()).unwrap();}
 assert_eq!(body(call(&mut read,&mut write,5,"inspect_document",json!({})).await),before);body(call(&mut read,&mut write,6,"history",json!({})).await);assert_ne!(body(call(&mut read,&mut write,7,"inspect_document",json!({})).await)["state_id"],before["state_id"]);body(call(&mut read,&mut write,8,"history",json!({"redo":true})).await);assert_eq!(body(call(&mut read,&mut write,9,"inspect_document",json!({})).await),before);if mode==2{assert_eq!(fs::read(&path).unwrap(),bytes);}drop(read);drop(write);server.await.unwrap();
 }
 }).await.expect("bounded read-only real rmcp controls");});
}
