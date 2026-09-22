<!-- SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me) -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Security policy

## Supported versions

Security fixes target the latest released version and the current `main` branch.
Older versions may require an upgrade; there is no guaranteed backport schedule.

## Report a vulnerability privately

Email **mccodeman@proton.me** with a description, affected version or commit,
impact, reproduction steps, and a minimal demo file if relevant. Do not open a
public issue for an unpatched vulnerability. Avoid sending passwords, tokens,
private command history, or sensitive terminal output.

The maintainer will investigate and coordinate disclosure and a fix with the
reporter. This volunteer project does not promise a response or remediation SLA.
If you do not receive a response, send a follow-up to the same address.

## Trust model

Demo commands run in real shells with your user's permissions. Review unfamiliar
TOML files before executing or typing cues. Shell configuration can also execute
startup files. nysos is not a sandbox. URL opening invokes a browser on the host
running nysos, which may be a remote machine in an SSH session.

Dependency inventories are provided as SPDX SBOMs. An SBOM is an inventory, not a
claim that a release is free of vulnerabilities. See [the development guide](docs/development.md)
for build, release, and Sigstore signature verification instructions.
