Little script to convert excalidraw diagrams into promptable representations.

In addition to a basic mermaid flow chart conversion, this cli also handles building hierarchies from nested nodes and inferring bindings by proximity even if not linked properly in the source diagram.

```
cargo build
./target/debug/excalidraw_to_prompt --path <path_to_excalidraw_export>
```
