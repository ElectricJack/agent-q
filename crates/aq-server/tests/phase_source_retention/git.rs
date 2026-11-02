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
    child: std::process::Child,
    leader: Process,
    closed: bool,
}
impl Owner {
    async fn start(parent: &Path, label: &str) -> Self {
        let root = parent.join(label);
        let socket = parent.join(format!("{label}.sock"));
        // Default free-space policy is deliberately preserved: no lowering flag.
        use std::os::unix::process::CommandExt;
        let mut child = std::process::Command::new(std::env::var("AQ_TEST_GIT_CANDIDATE").unwrap())
            .args([
                "--state-dir",
                root.to_str().unwrap(),
                "--git-executable",
                &std::env::var("AQ_TEST_GIT_EXECUTABLE").unwrap(),
                "--listen",
                socket.to_str().unwrap(),
                "serve",
            ])
            .process_group(0)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        // std::Child deliberately keeps the leader UNREAPED until every
        // descendant is drained: its kernel PID/PGID cannot be reused meanwhile.
        let leader = match process(child.id() as i32) {
            Some(leader) => leader,
            None => {
                // process_group(0) completed before exec and std::Child is
                // still unreaped, so this freshly allocated group is owned.
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(child.id() as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
                let _ = child.wait();
                panic!("fixture requires observable owned foreground leader");
            }
        };
        assert_eq!(leader.group, leader.pid);
        let owner = Self {
            root,
            socket,
            child,
            leader,
            closed: false,
        };
        println!(
            "SOURCE_RETENTION_OWNER group={} start={} session={} root={}",
            owner.leader.pid,
            owner.leader.start,
            owner.leader.session,
            owner.root.display()
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            while !owner.socket.exists() {
                let current = process(owner.leader.pid).expect("unreaped leader stays owned");
                assert_eq!(current.start, owner.leader.start);
                assert_ne!(
                    current.state, 'Z',
                    "Git owner refused startup; default policy retained"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        owner.check_descendants().unwrap();
        owner
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
    fn check_descendants(&self) -> Result<(), String> {
        let rows = processes()?;
        let current = rows
            .iter()
            .find(|p| p.pid == self.leader.pid)
            .ok_or("owned unreaped leader missing")?;
        if current.start != self.leader.start
            || current.group != self.leader.pid
            || current.session != self.leader.session
        {
            return Err("owned leader identity changed".into());
        }
        let mut owned = std::collections::BTreeSet::from([self.leader.pid]);
        let mut complete = false;
        for _ in 0..64 {
            let old = owned.len();
            for p in &rows {
                if owned.contains(&p.parent) {
                    owned.insert(p.pid);
                }
            }
            if old == owned.len() {
                complete = true;
                break;
            }
        }
        if !complete {
            return Err("fixture descendant depth bound exceeded".into());
        }
        for p in &rows {
            if owned.contains(&p.pid)
                && (p.group != self.leader.pid || p.session != self.leader.session)
            {
                return Err("Git descendant escaped owned foreground group/session".into());
            }
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), String> {
        if self.closed {
            return Ok(());
        }
        self.check_descendants()?;
        use nix::{
            sys::signal::{Signal, killpg},
            unistd::Pid,
        };
        let group = Pid::from_raw(self.leader.pid);
        killpg(group, Signal::SIGTERM).map_err(|e| e.to_string())?;
        // Retain the original std::Child unreaped while signaling/draining. A
        // repeated signal is only sent after exact start-time/PGID revalidation.
        for (limit, force) in [
            (std::time::Duration::from_secs(3), false),
            (std::time::Duration::from_secs(2), true),
        ] {
            if force {
                self.check_descendants()?;
                killpg(group, Signal::SIGKILL).map_err(|e| e.to_string())?;
            }
            let deadline = std::time::Instant::now() + limit;
            while std::time::Instant::now() < deadline {
                let rows = processes()?;
                let leader = rows
                    .iter()
                    .find(|p| p.pid == self.leader.pid)
                    .ok_or("unreaped leader missing during drain")?;
                if leader.start != self.leader.start
                    || leader.group != self.leader.pid
                    || leader.session != self.leader.session
                {
                    return Err("leader changed during owned drain".into());
                }
                if leader.state == 'Z'
                    && !rows
                        .iter()
                        .any(|p| p.group == self.leader.pid && p.pid != self.leader.pid)
                {
                    self.child.wait().map_err(|e| e.to_string())?;
                    self.closed = true;
                    println!("SOURCE_RETENTION_OWNER_DRAINED group={}", self.leader.pid);
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        Err("owned Git foreground group failed bounded drain".into())
    }
    pub async fn stop(&mut self) {
        self.close()
            .expect("drain every owned Git descendant before root teardown");
    }
    pub fn artifact(&self, value: &Value) -> PathBuf {
        let id: Uuid = serde_json::from_value(value["artifact_handle"].clone()).unwrap();
        self.root.join("artifacts").join(id.to_string())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!(
                "SOURCE_RETENTION_OWNER_CLEANUP_FAILED group={} root={} error={error}",
                self.leader.pid,
                self.root.display()
            );
        }
    }
}
#[derive(Clone)]
struct Process {
    pid: i32,
    parent: i32,
    group: i32,
    session: i32,
    start: u64,
    state: char,
}
fn process(pid: i32) -> Option<Process> {
    let bytes = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, tail) = bytes.rsplit_once(") ")?;
    let fields = tail.split_whitespace().collect::<Vec<_>>();
    Some(Process {
        pid,
        parent: fields.get(1)?.parse().ok()?,
        group: fields.get(2)?.parse().ok()?,
        session: fields.get(3)?.parse().ok()?,
        start: fields.get(19)?.parse().ok()?,
        state: fields.first()?.chars().next()?,
    })
}
fn processes() -> Result<Vec<Process>, String> {
    let mut rows = vec![];
    let mut count = 0;
    for entry in std::fs::read_dir("/proc").map_err(|e| e.to_string())? {
        count += 1;
        if count > 16384 {
            return Err("fixture process observation bound exceeded".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>()
            && let Some(row) = process(pid)
        {
            rows.push(row);
        }
    }
    Ok(rows)
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
