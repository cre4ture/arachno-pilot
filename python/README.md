# Python sidecar

Keep Python focused on tasks where it shines:

- dataset tooling
- experiment scripts
- training and evaluation
- model export back into the Rust runtime

Do not put the fast actuator loop here. Let Rust stay in charge of motion authority and safety.

Useful local utility:

- `python3 -m arachno_ml.codex_quota` reads the active Codex subscription rate-limit snapshot through the installed `codex` app-server
- `python3 -m arachno_ml.claude_quota` reads the active Claude Code subscription rate-limit snapshot through the installed `claude` CLI statusline feed
