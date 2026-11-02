use crate::common::presence;
use aq_admission::{Digest, ObjectFormat, ObjectId, SourceSelection};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

pub async fn git(path: &Path, args: &[&str]) -> String {
    let command = std::env::var("AQ_TEST_GIT_EXECUTABLE").expect("explicit pinned Git required");
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(command)
            .current_dir(path)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.test")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.test")
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn pin(name: &str, expected: &str) {
    use sha2::{Digest as _, Sha256};
    use std::io::Read;
    let path = std::env::var(name).expect("explicit executable path required");
    let wanted = std::env::var(expected).expect("explicit executable SHA256 required");
    assert!(Path::new(&path).is_absolute());
    let mut stream = std::fs::File::open(path).unwrap();
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let n = stream.read(&mut bytes).unwrap();
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    assert_eq!(
        format!("{:x}", hash.finalize()),
        wanted,
        "qualified executable changed"
    );
}
pub struct Owner {
    pub root: PathBuf,
    pub socket: PathBuf,
    child: tokio::process::Child,
}
impl Owner {
    async fn start(parent: &Path, label: &str) -> Self {
        let root = parent.join(label);
        let socket = parent.join(format!("{label}.sock"));
        // Default free-space policy is deliberately preserved: no lowering flag.
        let mut child =
            tokio::process::Command::new(std::env::var("AQ_TEST_GIT_CANDIDATE").unwrap())
                .args([
                    "--state-dir",
                    root.to_str().unwrap(),
                    "--git-executable",
                    &std::env::var("AQ_TEST_GIT_EXECUTABLE").unwrap(),
                    "--listen",
                    socket.to_str().unwrap(),
                    "serve",
                ])
                .kill_on_drop(true)
                .stdout(Stdio::null())
                .spawn()
                .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if socket.exists() {
                    break;
                }
                assert!(
                    child.try_wait().unwrap().is_none(),
                    "Git owner refused startup; retain its default policy"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        Self {
            root,
            socket,
            child,
        }
    }
    pub async fn call(&self, repo: Uuid, operation: &str, params: Value) -> Value {
        let id = Uuid::new_v4();
        let request = json!({"protocol_version":1,"request_id":id.to_string(),"operation_id":id,"operation":operation,"repository_id":repo,"expected_binding_generation":if operation=="repo.register"{0}else{1},"params":params});
        let stream = tokio::net::UnixStream::connect(&self.socket).await.unwrap();
        let mut reader = BufReader::new(stream);
        reader
            .get_mut()
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(10), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        assert!(line.len() < 1024 * 1024);
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["request_id"], id.to_string());
        assert_eq!(reply["ok"], true, "{reply}");
        reply["result"].clone()
    }
    pub async fn stop(&mut self) {
        self.child.kill().await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), self.child.wait())
            .await
            .unwrap()
            .unwrap();
    }
    pub fn artifact(&self, value: &Value) -> PathBuf {
        let id: Uuid = serde_json::from_value(value["artifact_handle"].clone()).unwrap();
        self.root.join("artifacts").join(id.to_string())
    }
}
pub struct Actual {
    pub central: Owner,
    pub worker: Owner,
    pub repo: Uuid,
    pub base: String,
    pub workspace: Value,
}
impl Actual {
    pub async fn prepare(ready: &mut presence::Case) -> Self {
        pin("AQ_TEST_GIT_CANDIDATE", "AQ_TEST_GIT_CANDIDATE_SHA256");
        pin("AQ_TEST_GIT_EXECUTABLE", "AQ_TEST_GIT_EXECUTABLE_SHA256");
        let c = &mut ready.c;
        let parent = c.env.env._dir.path();
        let seed = parent.join("source-seed");
        std::fs::create_dir(&seed).unwrap();
        git(&seed, &["init", "-b", "main"]).await;
        std::fs::write(seed.join("source"), "base fixture bytes\n").unwrap();
        git(&seed, &["add", "."]).await;
        git(&seed, &["commit", "-m", "base"]).await;
        let base = git(&seed, &["rev-parse", "HEAD"]).await;
        let repo = c.env.frozen.project.binding.repository_id;
        let central = Owner::start(parent, "rc").await;
        central.call(repo,"repo.register",json!({"identity":"phase-retention-actual-fixture","mode":"central","remote_url":seed,"allow_file_transport":true,"allowed_refs":["refs/heads/main"],"allowed_targets":["refs/heads/main"]})).await;
        central.call(repo, "repo.sync", json!({})).await;
        let oid = json!({"algorithm":"sha1","hex":base});
        let work = central
            .call(repo, "workspace.create", json!({"base":oid}))
            .await;
        let checkpoint=central.call(repo,"workspace.checkpoint",json!({"workspace_id":work["workspace_id"],"expected_generation":1,"expected_head":oid,"quiescent":true,"mode":"head"})).await;
        let export=central.call(repo,"artifact.export",json!({"base":oid,"source":oid,"retention_handle":checkpoint["retention_handle"],"confirmed_prerequisites":[]})).await;
        let worker = Owner::start(parent, "rw").await;
        worker.call(repo,"repo.register",json!({"identity":"phase-retention-actual-fixture","mode":"worker","binding_generation":1})).await;
        let artifact: Uuid = serde_json::from_value(export["artifact_handle"].clone()).unwrap();
        let incoming = worker
            .root
            .join("artifacts/incoming")
            .join(artifact.to_string());
        std::fs::create_dir_all(&incoming).unwrap();
        for file in ["manifest.json", "objects.bundle"] {
            std::fs::copy(central.artifact(&export).join(file), incoming.join(file)).unwrap();
        }
        worker
            .call(repo, "artifact.import", json!({"artifact_handle":artifact}))
            .await;
        let workspace = worker
            .call(repo, "workspace.create", json!({"base":oid}))
            .await;
        let mut config = (*c.env.env.state.config).clone();
        let mut profiled = (**config.profiled_execution.as_ref().unwrap()).clone();
        profiled.revision += 1;
        profiled.projects[0].retention_handle =
            serde_json::from_value(checkpoint["retention_handle"].clone()).unwrap();
        profiled.projects[0].source = SourceSelection::Immutable {
            object: ObjectId {
                format: ObjectFormat::Sha1,
                hex: base.clone(),
            },
        };
        config.profiled_execution = Some(Arc::new(profiled));
        c.env.env.state.config = Arc::new(config);
        c.env.env.state.source_retention = Some(Arc::new(
            aq_server::source_retention::Runtime::new(
                aq_server::source_retention::SourceRetentionConfig {
                    git_root: central.root.clone(),
                    git_socket: central.socket.clone(),
                    staging_root: parent.join("retention-staging"),
                    max_staging_bytes: 8 * 1024 * 1024,
                    max_bundle_bytes: 1024 * 1024,
                    bulk_concurrency: 1,
                    incomplete_ttl_secs: 60,
                },
            )
            .unwrap(),
        ));
        Self {
            central,
            worker,
            repo,
            base,
            workspace,
        }
    }
    pub async fn export_changed(&self) -> (Value, Vec<u8>, Vec<u8>) {
        let checkpoint=self.worker.call(self.repo,"workspace.checkpoint",json!({"workspace_id":self.workspace["workspace_id"],"expected_generation":1,"expected_head":{"algorithm":"sha1","hex":self.base},"quiescent":true,"mode":"all","commit":{"name":"Fixture","email":"fixture@example.test","message":"actual changed checkpoint"}})).await;
        assert_ne!(checkpoint["source"]["hex"], self.base);
        let export=self.worker.call(self.repo,"artifact.export",json!({"base":{"algorithm":"sha1","hex":self.base},"source":checkpoint["source"],"retention_handle":checkpoint["retention_handle"],"confirmed_prerequisites":[]})).await;
        let path = self.worker.artifact(&export);
        let manifest = std::fs::read(path.join("manifest.json")).unwrap();
        let bundle = std::fs::read(path.join("objects.bundle")).unwrap();
        assert!(manifest.len() < 65536 && !bundle.is_empty() && bundle.len() < 1024 * 1024);
        assert_eq!(
            export["manifest"]["bundle_sha256"],
            Digest::of(&bundle).as_str()
        );
        (export, manifest, bundle)
    }
}
