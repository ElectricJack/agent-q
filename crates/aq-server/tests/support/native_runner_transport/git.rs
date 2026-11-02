//! Explicit fixture-operator provisioning required by native hydration's
//! already-registered-repository profile. No agent input or generic RPC proxy.
use super::*;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::Duration,
};
use uuid::Uuid;

fn registration(central: &Value, operation: Uuid) -> Result<Value> {
    let repository: Uuid = serde_json::from_value(central["repository_id"].clone())?;
    let identity = central["identity"]
        .as_str()
        .context("actual central identity absent")?;
    let generation = central["binding_generation"]
        .as_u64()
        .context("actual central generation absent")?;
    ensure!(
        repository.get_version_num() == 4
            && operation.get_version_num() == 4
            && !identity.is_empty()
            && identity.len() <= 128
            && identity
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:@/".contains(&b))
            && generation > 0
            && central["mode"] == "central"
            && central["object_format"] == "sha1",
        "exact actual central repository required"
    );
    Ok(
        json!({"protocol_version":1,"request_id":operation,"operation_id":operation,
        "operation":"repo.register","repository_id":repository,"expected_binding_generation":0,
        "params":{"binding":{"identity":identity,"mode":"worker","binding_generation":generation,"object_format":"sha1"}}}),
    )
}
fn exchange(socket: &Path, request: &Value) -> Result<Value> {
    let mut stream = UnixStream::connect(socket).context("fixture Git provisioning unavailable")?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let bytes = serde_json::to_vec(request)?;
    ensure!(
        bytes.len() <= 2048,
        "bounded repository provisioning request"
    );
    stream.write_all(&bytes)?;
    stream.write_all(b"\n")?;
    let mut body = Vec::new();
    BufReader::new(stream)
        .take(65537)
        .read_until(b'\n', &mut body)?;
    ensure!(
        body.len() <= 65536 && body.last() == Some(&b'\n'),
        "bounded repository provisioning reply"
    );
    let reply: Value = serde_json::from_slice(&body)?;
    ensure!(
        reply["protocol_version"] == 1
            && reply["request_id"] == request["request_id"]
            && reply["ok"] == true
            && reply["result"].is_object()
            && reply.get("error").is_none()
            && (request.get("operation_id").is_none()
                || reply["operation_id"] == request["operation_id"]),
        "repository provisioning refused; preserve exact operation"
    );
    Ok(reply["result"].clone())
}
impl PrivateState {
    /// Call in a blocking job while ordinary run_once is still paused BEFORE
    /// heartbeat/Poll. A fresh operation requires expected generation zero;
    /// existing bindings refuse, with no update/delete/adoption fallback.
    pub fn provision_worker_repository(&self, central: &Value, operation: Uuid) -> Result<Value> {
        let request = registration(central, operation)?;
        let socket = PathBuf::from(format!(
            "/proc/thread-self/fd/{}/git.sock",
            self.root.as_raw_fd()
        ));
        let metadata = fs::symlink_metadata(&socket)?;
        ensure!(
            metadata.file_type().is_socket() && metadata.uid() == unsafe { libc::geteuid() },
            "owned fixture Git socket required"
        );
        let registered = exchange(&socket, &request)?;
        let current = exchange(
            &socket,
            &json!({"protocol_version":1,"request_id":Uuid::new_v4(),
            "operation":"repo.show","repository_id":request["repository_id"],
            "expected_binding_generation":central["binding_generation"],"params":{}}),
        )?;
        for result in [&registered, &current] {
            ensure!(
                result["repository_id"] == central["repository_id"]
                    && result["binding_generation"] == central["binding_generation"]
                    && result["identity"] == central["identity"]
                    && result["mode"] == "worker"
                    && result["object_format"] == "sha1",
                "exact provisioned worker repository required"
            );
        }
        ensure!(
            registered == current,
            "provisioned worker repository changed"
        );
        Ok(json!({"operation_id":operation,"registration":registered,"current":current}))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_preserves_actual_binding_and_never_copies_remote_authority() {
        let operation = Uuid::new_v4();
        let mut central = json!({"repository_id":Uuid::new_v4(),"identity":"aq-gate04-actual-source",
            "binding_generation":7,"mode":"central","object_format":"sha1","remote_url":"never forwarded"});
        let request = registration(&central, operation).unwrap();
        assert_eq!(request["expected_binding_generation"], 0);
        assert_eq!(request["repository_id"], central["repository_id"]);
        assert_eq!(
            request["params"]["binding"],
            json!({"identity":"aq-gate04-actual-source","mode":"worker","binding_generation":7,"object_format":"sha1"})
        );
        assert_eq!(request["operation_id"], operation.to_string());
        central["mode"] = json!("worker");
        assert!(registration(&central, operation).is_err());
        central["mode"] = json!("central");
        central["binding_generation"] = json!(0);
        assert!(registration(&central, operation).is_err());
    }
    #[test]
    fn held_state_socket_provisions_exact_binding_then_reads_current_and_refuses_conflict() {
        use std::os::unix::net::UnixListener;
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("git.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let central = json!({"repository_id":Uuid::new_v4(),"identity":"aq-gate04-actual-source",
            "binding_generation":1,"mode":"central","object_format":"sha1"});
        let mut worker = central.clone();
        worker["mode"] = json!("worker");
        let expected = central.clone();
        let thread = std::thread::spawn(move || {
            for (index, operation) in ["repo.register", "repo.show", "repo.register"]
                .into_iter()
                .enumerate()
            {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["operation"], operation);
                assert_eq!(request["repository_id"], expected["repository_id"]);
                let mut reply = if index == 2 {
                    json!({"protocol_version":1,"request_id":request["request_id"],"ok":false,"error":{"code":"binding_conflict","message":"not exported"}})
                } else {
                    json!({"protocol_version":1,"request_id":request["request_id"],"ok":true,"result":worker})
                };
                if operation == "repo.register" {
                    assert_eq!(request["expected_binding_generation"], 0);
                    assert_eq!(request["params"]["binding"]["mode"], "worker");
                    reply["operation_id"] = request["operation_id"].clone();
                }
                stream
                    .write_all(&serde_json::to_vec(&reply).unwrap())
                    .unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });
        let state = PrivateState {
            root: File::open(root.path()).unwrap(),
        };
        let receipt = state
            .provision_worker_repository(&central, Uuid::new_v4())
            .unwrap();
        assert_eq!(receipt["registration"], receipt["current"]);
        // A refused public owner response must never become an adopted binding.
        // Use a separate literal exchange so the test needn't fabricate state.
        let request = registration(&central, Uuid::new_v4()).unwrap();
        assert!(exchange(&socket, &request).is_err());
        thread.join().unwrap();
    }
}
