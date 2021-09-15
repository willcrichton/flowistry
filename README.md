# Flowistry

[![ci](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml/badge.svg)](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml)

Flowistry is a VSCode extension that helps developers understand Rust programs. Flowistry uses [dataflow analysis](https://en.wikipedia.org/wiki/Data-flow_analysis) and [pointer analysis](https://en.wikipedia.org/wiki/Pointer_analysis) to analyze Rust programs at a deeper semantic level than just types can offer. Flowistry's capabilities are:

### Backward slicing

A [backward slice](https://en.wikipedia.org/wiki/Program_slicing) identifies every piece of code that affects a value of interest. For example, let's say you're debugging an assertion failure on `x`. Then you could compute the backward slice of `x` to rule out lines of code that don't influence its value, as shown here:

![Screen Shot 2021-09-15 at 3 21 22 PM](https://user-images.githubusercontent.com/663326/133517705-8763f437-33d9-4451-8fad-ec224ddb2ad7.png)


### Forward slicing

A forward slice identifiers every piece of code that is affected by a value of interest. For example, let's say you have a program that times a calculation, and you want to find all the code related to timing. You could compute a forward slice of the timer to rule out the actual calculation:

![Screen Shot 2021-09-15 at 3 24 45 PM](https://user-images.githubusercontent.com/663326/133518019-4b2b03f2-5cb3-4e93-875d-bc2bba463d71.png)

## Function effects

A function's effects are either inputs that it mutates, or values that it returns. The function effects panel helps identify lines of code that either mutate arguments or return values. Selecting an effect then shows the backward slice of that effect.

![demo mp4](https://user-images.githubusercontent.com/663326/133518170-cfc0e12b-6be3-4180-a661-418d3ccb5d2b.gif)

## Installation

You can install Flowistry from the [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=wcrichton.flowistry). Alternatively, you can install it from source:

### Flowistry server

#### From Cargo

```
# Install flowistry binaries
rustup toolchain install nightly -c rust-src,rustc-dev,llvm-tools-preview
git clone https://github.com/willcrichton/flowistry
cd flowistry
cargo +nightly install --path .

# Install vscode extension
cd ide
npm run install
npm run build
ln -s $(pwd) ~/.vscode/extensions/flowistry
```

## Usage

TODO: explain flowistry keybindings
