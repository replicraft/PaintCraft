use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

#[derive(Debug)]
pub(super) struct Client {
    child: Child,
    stdin: ChildStdin,
    messages: mpsc::Receiver<Value>,
    next: u64,
}
impl Client {
    pub(super) fn at(executable: &std::path::Path) -> Self {
        let mut child = Command::new(executable)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, messages) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.unwrap();
                let value = serde_json::from_str(&line).expect("stdout must contain only JSON-RPC");
                if tx.send(value).is_err() {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            messages,
            next: 0,
        };
        let initialized = client.request("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"painting-test","version":"1"}}));
        assert_eq!(initialized["serverInfo"]["name"], "paintcraft");
        client.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        client
    }
    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }
    pub(super) fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        self.send(json!({"jsonrpc":"2.0","id":self.next,"method":method,"params":params}));
        loop {
            let message = self
                .messages
                .recv_timeout(Duration::from_secs(30))
                .expect("MCP response timed out");
            if message["id"] == self.next {
                assert!(message.get("error").is_none(), "{message}");
                return message["result"].clone();
            }
        }
    }
    pub(super) fn call(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name":name,"arguments":args}))
    }
    // Shared support is compiled by both protocol and installed-helper suites.
    #[allow(dead_code)]
    pub(super) fn cancel_call(&mut self, name: &str, args: Value) {
        self.next += 1;
        self.send(json!({"jsonrpc":"2.0","id":self.next,"method":"tools/call","params":{"name":name,"arguments":args}}));
        self.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":self.next,"reason":"test cancellation"}}));
    }
    pub(super) fn ok(&mut self, name: &str, args: Value) -> Value {
        let result = self.call(name, args);
        assert_ne!(result["isError"], true, "{result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
