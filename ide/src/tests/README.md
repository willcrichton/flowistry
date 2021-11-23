# Extension Unit Tests
This directory contains unit tests for the Flowistry VS Code extension.

## `mock_project/`
The `mock_project/` directory contains a Rust project which we can open in VS Code and on which we can perform Flowistry commands.

## Install tests
Found in `install/`.

Install tests load the extension and wait for the VS Code Flowistry commands to become available. Because the tests start without Flowistry installed, the extension must download the binaries from the latest release on Github. To instead install from the current Flowistry build, we start a proxy server that replaces requests to `.zip` files on Github with a local `.zip` of the current build.

## Slice tests
Found in `slice/`. These test the extension's display of the slicer output (text highlight or text decorations).

### Selection
Found in `slice/select.test.ts`.

Slice selection tests open `mock_project/` in VS Code and execute Flowistry select commands (`flowistry.forward_select` or `flowistry.backward_select`) on highlighted values. The tests compare the resulting highlighted text in VS Code with the expected slice computed from running `cargo flowistry` on the highlighted value.

For example, given the following function:
```rust
fn main() {
    let x = 1;
    let y = if true { 1 } else { 2 };
    x;
}
```

A `backward_select` test would select `x` on line 4, run `flowistry.backward_select`, and expect the following to be selected:
```rust
    let x = 1;
    x;
```

`slice/mock_data/slices.ts` contains the slices to perform.

### Decoration
Because of unit testing limitations for VS Code text decorations, they aren't currently tested. Although text decorations and selections should both distinguish the same slices of text, decorations should be tested in `slice/decorations.test.ts` in the future.
