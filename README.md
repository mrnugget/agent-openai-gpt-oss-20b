# agent-openai-gpt-oss-20b

A tiny Rust CLI chat agent that talks to an OpenAI-compatible API and can read/list/edit files via simple "tools".

## Requirements
- Rust toolchain (Cargo)
- An OpenAI-compatible HTTP server running at `http://localhost:1234/v1/` that serves the model `openai/gpt-oss-20b`
  - No API key is used (the code sets a placeholder), so the server should not require auth.

## Run
```bash
cargo run
```
This launches an interactive REPL (`exit` to quit).

## What it does
- Sends conversation turns to your local API using `openai/gpt-oss-20b`.
- Parses responses using Harmony and supports tool calls to:
  - `list_files` — list directory contents
  - `read_file` — print a file’s contents
  - `edit_file` — replace text in a file or write new contents when `old_str` is empty

The base URL and model are hard-coded in [`src/main.rs`](file:///Users/mrnugget/work/agent-openai-gpt-oss-20b/src/main.rs). Adjust there if needed.

## Examples
- Simple chat:
  - You: `What is 2 + 2?`
  - Assistant: `4`

- Use a tool (directory listing):
  - You: `List the files in the current directory`
  - The agent may show a tool call and then return a JSON array of entries.

- Read a file:
  - You: `Open README.md`
  - Assistant will call `read_file` and print its contents.

- Edit or create a file:
  - You: `Replace "foo" with "bar" in notes.txt` (the agent will translate that to an `edit_file` call)

## Notes
- See [`Cargo.toml`](file:///Users/mrnugget/work/agent-openai-gpt-oss-20b/Cargo.toml) for dependencies (`openai_api_rust`, `openai-harmony`).
- Tools are executed on your local filesystem relative to the working directory.
