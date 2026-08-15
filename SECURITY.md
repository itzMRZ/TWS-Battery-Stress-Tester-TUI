# Security

tws-tester runs on the local machine. It talks to the host Bluetooth and audio stacks. It does not open a network server.

`tws-tester --update` and the install scripts download the latest GitHub release over HTTPS and refuse to replace the binary unless the SHA-256 matches the published `.sha256` file.

Report vulnerabilities privately via [GitHub Security Advisories](https://github.com/itzMRZ/TWS-Battery-Stress-Tester-TUI/security/advisories/new). Do not file a public issue for a still-private hole.
