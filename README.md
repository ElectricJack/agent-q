# Agent Q

The new Agent Queue foundation, developed as a fresh start alongside the existing installation. V1 migration is not a launch requirement. Any optional owner-specific data transfer belongs in an external one-off script outside aq-graph.

Start with [Phase 1: Infrastructure foundation](docs/specs/0001-infrastructure-foundation.md).

The initial deployment targets a Bluehost self-managed VPS with Docker Compose and Cloudflare DNS, Tunnel, and Access. Phase one covers hosting, login, worker connectivity, development, deployment, self-update, and recovery.

This repository currently contains the draft specification only. Task scheduling, agent execution, and integration workflows are later phases.
