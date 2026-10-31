# aq-session candidate

The independent candidate binary consumes the same authenticated human relay as
HTTP clients. Production requires an HTTPS origin and an owner-only Cloudflare
Access JWT file. Explicit `loopback-development` builds accept `--dev-email` only
for a loopback development origin. It contains no worker credentials or SSH,
process launch, fallback shell, transport retry or automatic input resubmission.

`aq-session --origin https://aq.example --cf-access-token-file /private/token inspect TERMINAL`
inspects an exact registered identity. `attach TERMINAL` watches for 30 seconds
(`--seconds 1..300`). It emits bounded JSON snapshots and output frames; control
characters remain escaped. This is a relay client, not a qualified terminal
renderer. It neither interprets an opaque renderer state nor executes ANSI.

`attach TERMINAL --control` explicitly acquires an expiring controller. It waits
for the peer's exact fence acknowledgement, then accepts at most4096 literal stdin
bytes per command (ordinary TTY canonical line behavior), waits for that receipt,
and propagates bounded TTY size changes. It renews the lease through a new fenced
generation and uses server-issued deterministic input epochs where enabled.
Network/input-reply uncertainty stops input. Unknown delivery requires explicit
fresh capture and resolution; attach never guesses that injection failed. The
controller expires independently if the client stalls or exits. Ctrl+C/crash may
leave a command unknown; inspect its printed operation ID before further input.

`receipt TERMINAL OPERATION` reads metadata. `mutate --request /private/intent.json`
submits exactly one principal-free typed intent, including explicit capture and
unknown resolution. The intent must be a private owner-only single-link regular
file, at most64KiB. Protect raw literal input accordingly. No mutation is retried.

`logs ATTEMPT --after N --through SNAPSHOT --limit 1..16` reads one immutable
snapshot page and prints its continuation metadata. `usage ATTEMPT` preserves
known subtotals, unknown counters, completeness and source accounting markers;
category totals may overlap and are not summed. Both use the frozen
`aq-observation-api0.1.0` bounded typed decoders and check the requested attempt.
These server handlers remain separately qualified/integrated by AQ22.

The terminal server must be explicitly configured and installed, and the remote
peer must implement the negotiated AQ-owned relay contract. CON02/API0.1.0 bytes
are unchanged. Current qualification uses synthetic SQL/native authorities and
fake HTTP peers, not a live provider, tmux renderer or WRK14 bridge. Central
runtime registration is refused until its independent authority adapter exists.
Finite generic mutation/ACL/history quotas remain explicit; no unlimited attach,
500-agent capacity, product alias, deployment or provider qualification is claimed.
