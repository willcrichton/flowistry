# <img src="https://user-images.githubusercontent.com/663326/134070630-47b95f41-a4a7-4ded-a5cb-9884d1af2468.png" height="30" /> Flowistry: Powerful IDE Tools for Rust

[![tests](https://github.com/willcrichton/flowistry/actions/workflows/tests.yml/badge.svg)](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/flowistry.svg)](https://crates.io/crates/flowistry)

Flowistry is a VSCode extension that helps you understand Rust programs. Flowistry uses [dataflow analysis](https://en.wikipedia.org/wiki/Data-flow_analysis) and [pointer analysis](https://en.wikipedia.org/wiki/Pointer_analysis) to analyze Rust programs at a deeper level than just types can offer (e.g. as you can already find in [rust-analyzer](https://rust-analyzer.github.io/)).

**Flowistry is alpha software (see [Limitations](#limitations)).** I'm seeking early adopters to try it out and provide feedback! If you have questions or issues, please [file a Github issue](https://github.com/willcrichton/flowistry/issues), [join our Discord](https://discord.gg/XkcpkQn2Ah), or [DM @wcrichton on Twitter](https://twitter.com/wcrichton).

Currently, Flowistry's capabilities are:

### 1. Backward slice: find code that influences a value

Flowistry can compute a [backward static slice](https://en.wikipedia.org/wiki/Program_slicing) that identifies every piece of code that affects a value of interest. For example, let's say you're debugging an assertion failure on `x`. Then you could compute the backward slice of `x` to quickly rule out lines of code that don't influence its value, as shown here:

![demo1](https://user-images.githubusercontent.com/663326/134042737-0957a533-8c53-49b6-ba5b-d19de9a96d88.gif)

The green marker indicates the selected value, and the grey text indicates code with no influence on that value. Note that "influence" is a bit subtle --- for example, in the program:

```rust
if x > 0 {
  *y += 1;
}
```

Even though `x` isn't directly used to change `y`, we would say `x` influences `y` because a mutation to `y` happens conditionally based on the value of `x`.

<br>
<br>

### 2. Forward slice: find code that is influenced by a value

A forward static slice identifies every piece of code that is affected by a value of interest. For example, let's say you have a program that times a calculation, and you want to comment out all the code related to timing. You could compute a forward slice of the timer:

![demo2](https://user-images.githubusercontent.com/663326/134043212-f4263dc5-5f9b-432b-9e72-f57c1188b0c4.gif)

The timer doesn't affect the value of the computation, so `run_expensive_calculation` isn't part of the forward slice of `start`. In this example, Flowistry sets the user's selected text to the slice. Then the user can use other IDE features like bulk-commenting (⌘-/ in VSCode on macOS) on that selection.

Note that this example shows how slices are *transitive*: `start` influences `elapsed`, and `elapsed` influences `println`, so `start` influences `println`.

<br>
<br>

### 3. Function effects

A function's effects are either inputs that it mutates, or values that it returns. The function effects panel helps identify lines of code that either mutate arguments or that could return values. Selecting an effect then shows the backward slice of that effect. 

![demo mp4](https://user-images.githubusercontent.com/663326/133518170-cfc0e12b-6be3-4180-a661-418d3ccb5d2b.gif)

Like before, lines that are outside of a given slice are grayed out. But for this feature, lines that are _unique_ to a given slice are highlighted in orange. This way you can quickly focus on code that is only relevant to an effect of interest.

<br>
<br>

## Installation

You can install Flowistry from the [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=wcrichton.flowistry) or the [Open VSX Registry](https://open-vsx.org/extension/wcrichton/flowistry). In VSCode: 
* Go to the Extensions pane by clicking this button in the left margin: <img width="30" alt="Screen Shot 2021-09-20 at 9 30 43 AM" src="https://user-images.githubusercontent.com/663326/134039225-68d11dce-be71-4f33-8057-569346ef26bc.png">
* Search for "Flowistry" and then click "Install".
* Open a Rust workspace and wait for the tool to finish installing.


Alternatively, you can install it from source:

```
# Install flowistry binaries
git clone https://github.com/willcrichton/flowistry
cd flowistry
cargo install --path crates/flowistry_ide

# Install vscode extension
cd ide
npm install
npm run build
ln -s $(pwd) ~/.vscode/extensions/flowistry
```

## Usage

Flowistry has five commands:
* **Flowistry: Backward Highlight**: given a selected variable, this command highlights the backward slice of the variable.
* **Flowistry: Backward Select**: same as above, but this puts the slice in your selection rather than highlighting it. 
* **Flowistry: Forward Highlight** and **Flowistry: Forward Select**: same as above, but for forward slices than backward slices.
* **Flowistry: Effects**: given your cursor is within a particular function, this command opens the effects panel for that function.

You can invoke these commands either through the context menu, by right-clicking and opening the "Flowistry" sub-menu. Or you can open the Command Palette (⇧⌘P on Mac) and type the name of the command.

## Limitations

Flowistry is early-stage software. It inevitably has bugs in both the UI and the analysis. The UI may be unintuitive. The analysis, even if correct, may also be unintuitive.

Flowistry does not support all of Rust's features. Specifically:
* **Raw pointers:** Flowistry uses lifetimes to determine what a reference can point-to. However, raw pointers can point to anything and aren't tracked by the compiler. So Flowistry cannot detect the flow of information through a raw pointer. It can, however, detect information flow through a typed wrapper _around_ a raw pointer!
* **Some forms of interior mutability:** as a corollary to above, Flowistry does not track the aliases of data wrapped in types like `Rc<T>`. For example, in the program:
  ```rust
  let x = Rc::new(RefCell::new(1));
  let y = x.clone();
  *x.borrow_mut() = 2;
  ```
  Flowistry can detect that `x` is modified (because `borrow_mut` uses lifetimes to relate the `RefMut` to `RefCell`), but not that `y` is modified (because nothing statically says that `y` aliases `x`).

## FAQ

### rustup fails on installation

If rustup fails, especially with an error like "could not rename downloaded file", this is probably because Flowistry is running rustup concurrently with another tool (like rust-analyzer). Until [rustup#988](https://github.com/rust-lang/rustup/issues/988) is resolved, there is unfortunately no automated way around this. 

To solve the issue, go to the command line and run:

```
rustup toolchain install nightly-2021-10-08 -c rust-src -c rustc-dev -c llvm-tools-preview
```

> Note: double check the value of "channel" in `rust-toolchain.toml` if `nightly-2021-10-08` is no longer correct.

Then go back to VSCode and click "Continue" to let Flowistry continue installing.
