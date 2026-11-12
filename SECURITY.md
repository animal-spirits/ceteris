# Security policy

## Reporting a vulnerability

Report vulnerabilities privately through GitHub private vulnerability reporting: on the
repository page, open the Security tab and choose "Report a vulnerability". Do not open a public
issue or pull request for a suspected vulnerability.

Include the affected version or commit, steps to reproduce, and the impact you expect. We aim to
acknowledge a report within 7 days and to agree a disclosure date with you once a fix exists.

## Scope

In scope: the simulator crates, the desktop app, the command-line tools, scenario and save-file
parsing, and the pack fetch and build tooling (path traversal, unsafe deserialisation, memory
safety, arbitrary code execution, integrity checks on downloaded data).

Out of scope: determinism or calibration defects without a security impact (use a normal issue),
vulnerabilities in third-party data sources, and findings that need an already-compromised
machine.

## Supported versions

There are no releases yet. Until the first release, only the `main` branch is supported.
