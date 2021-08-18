# Flowistry

[![ci](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml/badge.svg)](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml)

Flowistry is an IDE extension that helps developers understand Rust programs. Flowistry uses [dataflow analysis](https://en.wikipedia.org/wiki/Data-flow_analysis) to analyze Rust programs at a deeper semantic level than just types can offer. Flowistry's capabilities are:

### Backward slicing

A [backward slice](https://en.wikipedia.org/wiki/Program_slicing) identifies every piece of code that affects a value of interest. For example, this screenshot shows the slice of `input` on line 14:

![Screen Shot 2021-03-22 at 2 00 43 PM](https://user-images.githubusercontent.com/663326/112676422-a51ce300-8e25-11eb-9195-2d6072f074bf.png)

The value of `buffer` affects the value of `input`, so `stdin.read_buffer(..)` and `buffer.clear()` are highlighted. The variable `count` does not affect the value of `input`, so  `let mut count = 0` and `count += 1` are not highlighted. 

### Forward slicing

TODO

## Installation

### Flowistry server

#### From Cargo

```
rustup toolchain install nightly -c rust-src,rustc-dev,llvm-tools-preview
cargo +nightly install flowistry
```

#### From source

```
git clone https://github.com/willcrichton/flowistry
cd flowistry
cargo install --path .
```

### VSCode extension

#### From source

```
cd ide
npm run vscode:prepublish
ln -s $(pwd) ~/.vscode/extensions/flowistry
```

## Usage

Flowistry contains two components: a program analyzer, and a VSCode extension that bridges the analyzer to a code editor. To try slicing a program, open a Rust project in VSCode and go to a Rust file. Select an expression you want to slice, then press ⌘+⇧+P, type "Slice", and hit enter. 
