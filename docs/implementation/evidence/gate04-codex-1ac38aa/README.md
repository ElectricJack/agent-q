# Actual acceptance/downloads; local hydration refused before Prepared

Source `1ac38aa810c74ef91cfb594dc912cdad4d0c5654`, immutable test SHA256 `88cad1ade141e65bffed5860354201b646cee2425925909dce117e11619bfc30` (106,532,920 bytes). Thirteen hostless checks and strict passed. The changed actual case failed in 38.15 seconds; no Codex process or provider request ran.

Genuine named-readonly preflight and real heartbeat/registration/scoped inventory passed. Actual Poll, refresh proposal and Accept returned HTTP200. Artifact/material and source status/manifest/bundle downloads returned HTTP200. The worker then exited during source hydration before Prepared/Execution. Format12/revision16 had acceptance but no prepared inputs, execution grant, native Running/Stopped/ECHILD, terminal ACK, output exchange, claim, source upload or B1. The 90-byte stderr is only the fixed generic CLI mask; it does not establish which local Git operation failed. No raw error/material/config is exported.

Exact sender pidfd observed exit; central Git reported DRAINED. Independent owned SID1082442 scan found zero remaining and zero unknown processes. Private root `/tmp/aq-g04-H6ZTn5` and all earlier negative pins/logs remain retained.

Source inspection identified an explicit unmet fixture prerequisite: WRK-08 native hydration requires an already registered exact worker repository; its repository operation intentionally calls repo.show. The fresh wrapper starts aq-git but never provisions a repository, whereas prior static fixtures did. The next diagnosed assembly correction performs explicit trusted outer-operator repo.register(expected generation0) then repo.show through the retained state directory, before heartbeat/Poll. It never adds implicit artifact-driven registration or changes ordinary runtime semantics. Exact failed operation remains unproven by this run's retained diagnostics.

Original execution20s, preparation/health limits, Git minimum, binary/bootstrap/policy pins remain unchanged. Earlier negatives are preserved, not replaced.
