# excalidraw_to_prompt

A CLI tool that converts [Excalidraw](https://excalidraw.com/) diagrams into LLM-friendly Mermaid flowcharts — so you can sketch an architecture and drop it straight into a prompt.

## Why

Excalidraw's JSON export is verbose and full of rendering metadata that's useless to an LLM. This tool distills it down to a clean Mermaid `flowchart TD` representation that language models understand natively.

## Features

- **Mermaid flowchart generation** — rectangles become nodes, arrows become directed edges (`-->`), and lines become undirected edges (`---`).
- **Hierarchy from nesting** — rectangles visually nested inside other rectangles are emitted as Mermaid `subgraph` blocks, with support for arbitrary nesting depth.
- **Proximity-based binding** — arrow endpoints that aren't formally bound to a node in the Excalidraw data are matched to the nearest node by edge distance (within a 50px threshold), so loosely-drawn diagrams still produce correct graphs.
- **Readable node IDs** — labels are converted to camelCase identifiers (e.g. `"User Service"` → `userService`) with smart deduplication that extends with extra words before falling back to numeric suffixes.
- **Edge labels** — text bound to an arrow or line is preserved as an edge label.
- **Tolerance for imprecision** — containment detection uses a 20px tolerance so slightly misaligned nested boxes are still recognized.

## Installation

Requires [Rust](https://www.rust-lang.org/tools/install) (edition 2024).

```/dev/null/sh#L1-2
cargo build --release
cp target/release/excalidraw_to_prompt /usr/local/bin/  # optional
```

## Usage

```/dev/null/sh#L1
excalidraw_to_prompt --path <path_to_excalidraw_file>
```

The tool reads an Excalidraw JSON export (`.excalidraw` file) and prints the Mermaid flowchart to stdout.

### Example

Given an Excalidraw diagram with three boxes — "Frontend", "API Gateway", and "Database" — connected by arrows labeled "REST" and "SQL":

```/dev/null/mermaid#L1-5
flowchart TD
    frontend["Frontend"]
    apiGateway["API Gateway"]
    database["Database"]
    frontend -->|"REST"| apiGateway
    apiGateway -->|"SQL"| database
```

Nested boxes produce subgraphs:

```/dev/null/mermaid#L1-7
flowchart TD
    subgraph backend["Backend"]
        authService["Auth Service"]
        userService["User Service"]
    end
    frontend["Frontend"]
    frontend --> authService
```

### Piping into a prompt

```/dev/null/sh#L1-4
echo "Here is my system architecture:

$(excalidraw_to_prompt --path ./design.excalidraw)

Suggest improvements for scalability." | pbcopy
```

## Supported Excalidraw elements

| Element     | Mermaid output          |
| ----------- | ----------------------- |
| Rectangle   | Node or subgraph        |
| Arrow       | Directed edge (`-->`)   |
| Line        | Undirected edge (`---`) |
| Text (bound to a shape) | Node label     |
| Text (bound to an arrow/line) | Edge label |
| Text (free-standing) | Standalone node  |

Deleted elements are automatically filtered out.

## Running tests

```/dev/null/sh#L1
cargo test
```
