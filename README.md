# Flowistry: Powerful IDE Tools for Rust

[![ci](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml/badge.svg)](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml)

Flowistry is a VSCode extension that helps you understand Rust programs. Flowistry uses [dataflow analysis](https://en.wikipedia.org/wiki/Data-flow_analysis) and [pointer analysis](https://en.wikipedia.org/wiki/Pointer_analysis) to analyze Rust programs at a deeper level than just types can offer (e.g. as you can find in [rust-analyzer](https://rust-analyzer.github.io/)).

**Flowistry is alpha software (see [Limitations](#limitations)) and only works on code compiled with nightly.** I'm seeking early adopters to try it out and provide feedback! If you have questions or issues, please [file a Github issue](https://github.com/willcrichton/flowistry/issues), [join our Discord](https://discord.gg/XkcpkQn2Ah), or [DM @wcrichton on Twitter](https://twitter.com/wcrichton).

Flowistry's capabilities are:

### Backward slicing

A [backward slice](https://en.wikipedia.org/wiki/Program_slicing) identifies every piece of code that affects a value of interest. For example, let's say you're debugging an assertion failure on `x`. Then you could compute the backward slice of `x` to rule out lines of code that don't influence its value, as shown here:

![demo1](https://user-images.githubusercontent.com/663326/134042737-0957a533-8c53-49b6-ba5b-d19de9a96d88.gif)

<br>
<br>

### Forward slicing

A forward slice identifiers every piece of code that is affected by a value of interest. For example, let's say you have a program that times a calculation, and you want to comment out all the code related to timing. You could compute a forward slice of the timer:

![demo2](https://user-images.githubusercontent.com/663326/134043212-f4263dc5-5f9b-432b-9e72-f57c1188b0c4.gif)

<br>
<br>

### Function effects

A function's effects are either inputs that it mutates, or values that it returns. The function effects panel helps identify lines of code that either mutate arguments or return values. Selecting an effect then shows the backward slice of that effect.

![demo mp4](https://user-images.githubusercontent.com/663326/133518170-cfc0e12b-6be3-4180-a661-418d3ccb5d2b.gif)

<br>
<br>

## Installation

You can install Flowistry from the [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=wcrichton.flowistry). 
* Go to the Extensions pane by clicking this button in the left margin: <img width="30" alt="Screen Shot 2021-09-20 at 9 30 43 AM" src="https://user-images.githubusercontent.com/663326/134039225-68d11dce-be71-4f33-8057-569346ef26bc.png">
* Search for "Flowistry" and then click "Install".
* Open a Rust workspace and wait for the tool to finish installing.


Alternatively, you can install it from source:

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

Flowistry has four commands:
* **Flowistry: Backward Highlight**: given a selected variable, this command highlights the backward slice of the variable.
* **Flowistry: Backward Select**: same as above, but this puts the slice in your selection rather than highlighting it. 
* **Flowistry: Forward Highlight** and **Flowistry: Forward Select**: same as above, but for forward slices than backward slices.
* **Flowistry: Effects**: given your cursor is within a particular function, this command opens the effects panel for that function.

You can invoke these commands either through the context menu, by right-clicking and opening the "Flowistry" sub-menu. Or you can open the Command Palette (⇧⌘P on Mac) and type the name of the command.

## Limitations

Flowistry is early-stage software. It inevitably has bugs in both the UI and the analysis. The UI may be unintuitive. The analysis, even if correct, may also be unintuitive. Additionally, Flowistry only works on code compiled with the nightly Rust compiler.
