# Rust Slicer

Rust Slicer is a [backwards static program slicer](https://en.wikipedia.org/wiki/Program_slicing). The tool uses static analysis to identify every statement that affects a particular value of interest. For example, this screenshot shows the slice of `input` on line 14:

![Screen Shot 2021-03-22 at 2 00 43 PM](https://user-images.githubusercontent.com/663326/112676422-a51ce300-8e25-11eb-9195-2d6072f074bf.png)

The value of `buffer` affects the value of `input`, so `stdin.read_buffer(..)` and `buffer.clear()` are highlighted. The variable `count` does not affect the value of `input`, so  `let mut count = 0` and `count += 1` are not highlighted. 

Rust Slicer is an early-stage, experimental research project. You should not expect the tool to work robustly. Any contributions are welcome!

## Installation

Currently, Rust Slicer only supports installation from source.

### From source

```
git clone https://github.com/willcrichton/rust-slicer
cd rust-slicer
cargo install --path .
cd extension
npm run vscode:prepublish
ln -s ~/.vscode/extensions/rust-slicer $(pwd)
```

## Usage

Rust Slicer contains two components: a program analyzer, and a VSCode extension that bridges the analyzer to a code editor. To try slicing a program, open a Rust project in VSCode and go to a Rust file. Select an expression you want to slice, then press ⌘+⇧+P, type "Slice", and hit enter. 
