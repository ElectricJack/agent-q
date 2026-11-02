# Pure helper EOF refusal, before worker

Source6dc386b, test SHA25650c499f5108e7ff1660fecaaf27e3b0b9d20b8c50f9ecb2cfc816ab3ed23999f. Actual case failed13.33s after successful real Graph/Git/project setup, recipe and owner installation. Root `/tmp/aq-g04-c4RWm7` contains the synthetic key and tool script, but no fixture socket or worker stderr: ordinary worker/Codex/admission did not start.

The finite peer's pure subprocess adapter retained stdin after Tokio's no-op ChildStdin.shutdown while its child read to EOF. The 2s bound refused. Source2bd74f0 moves the actual stdin owner into the writer, flushes and drops it before waiting for output; an actual pure Python EOF exchange regression passes. Work independently accepted that correction. Git printed DRAINED; independent SID1076685 scan found0remaining/0unknown. No provider/model/task-completion claim.
