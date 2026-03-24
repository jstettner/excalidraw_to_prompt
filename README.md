# excalidraw_to_prompt

A CLI that converts [Excalidraw](https://excalidraw.com/) diagrams into LLM-friendly flowcharts. Excalidraw's JSON export is verbose and full of rendering metadata that's useless to an LLM. This tool distills it down to a clean Mermaid `flowchart TD` representation that language models understand natively.

## Features

- **Hierarchy from nesting** — rectangles visually nested inside other rectangles are emitted as `subgraph` blocks, with support for arbitrary nesting depth.
- **Proximity-based binding** — arrow endpoints that aren't formally bound to a node in the Excalidraw data are matched to the nearest node by edge distance (within a 50px threshold), so loosely-drawn diagrams still produce correct graphs.
- **Tolerance for imprecision** — containment detection uses a 20px tolerance so slightly misaligned nested boxes are still recognized.

## Installation

```/dev/null/sh#L1-2
cargo build --release
cp target/release/excalidraw_to_prompt /usr/local/bin/  # optional
```

## Usage

```/dev/null/sh#L1
excalidraw_to_prompt --path <path_to_excalidraw_file>
```

The tool reads an Excalidraw JSON export (`.excalidraw` file) and prints the flowchart to stdout.

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

## Running tests

```/dev/null/sh#L1
cargo test
```
