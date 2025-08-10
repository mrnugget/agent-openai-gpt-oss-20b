# agent-openai-gpt-oss-20b

A tiny agent based on [How to Build an Agent](https://ampcode.com/how-to-build-an-agent).

Written in Rust. It's made to talk to `gpt-oss-20b` running locally, via an an OpenAI-compatible API.

<img width="664" height="1028" alt="screenshot-2025-08-09-18 33 04@2x" src="https://github.com/user-attachments/assets/e236fa86-3137-45fa-9735-9b49e1300cf0" />


# Run

```bash
cargo run
```

The base URL and model are hard-coded in [`src/main.rs`](file:///Users/mrnugget/work/agent-openai-gpt-oss-20b/src/main.rs). Adjust there if needed.
