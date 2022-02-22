# <img src="https://user-images.githubusercontent.com/663326/134070630-47b95f41-a4a7-4ded-a5cb-9884d1af2468.png" height="25" /> Flowistry: Information Flow in the IDE for Rust

[![tests](https://github.com/willcrichton/flowistry/actions/workflows/tests.yml/badge.svg)](https://github.com/willcrichton/flowistry/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/flowistry.svg)](https://crates.io/crates/flowistry)


Flowistry is a tool that analyzes the [information flow](https://en.wikipedia.org/wiki/Information_flow_(information_theory)) of Rust programs. Flowistry understands whether it's possible for one piece of code to affect another. Flowistry integrates into the IDE to provide a "focus mode" which helps you focus on the code that's related to your current task. 

For example, this GIF shows the focus mode when reading a function that unions two sets together:

<kbd>
<img src="https://user-images.githubusercontent.com/663326/155062086-4d58a149-86c8-4a48-a6aa-a07cf027641c.gif"  height=180 />
 </kbd>
 <br /><br />

When the user clicks a given variable or expression, Flowistry fades out all code that *does not influence* that code, and *is not influenced by* that code. For example, `orig_len` is not influenced by the for-loop, while `set.len()` is. 

Flowistry can be helpful when you're reading a function with a lot of code. For example, this GIF shows a real function in the Rust compiler. If you want to understand the role of a specific argument to the function, then Flowistry can filter out most of the code as irrelevant:

<kbd>
<img src="https://user-images.githubusercontent.com/663326/155062527-6b42f64a-3429-4572-860f-7c2e244691d6.gif" height=500 />
</kbd>
<br /><br />

**Table of contents**
* [Installation](#installation)
* [Usage](#usage)
    * [1. Startup](#1-startup)
    * [2. Entering focus mode](#2-entering-focus-mode)
    * [3. Setting a mark](#3-setting-a-mark)
    * [4. Selecting the focus region](#4-selecting-the-focus-region)
* [Limitations](#limitations)
    * [1. Flowistry does not completely handle interior mutability](#1-flowistry-does-not-completely-handle-interior-mutability)
    * [2. A focus region may include more code than you expect](#2-a-focus-region-may-include-more-code-than-you-expect)
    * [3. Not all code is selectable](#3-not-all-code-is-selectable)
* [FAQ](#faq)
    * [1. rustup fails on installation](#1-rustup-fails-on-installation)
    * [2. Why isn't Flowistry part of Rust Analyzer?](#2-why-isnt-flowistry-part-of-rust-analyzer)



## Installation 

Flowistry is available as a VSCode plugin. You can install Flowistry from the [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=wcrichton.flowistry) or the [Open VSX Registry](https://open-vsx.org/extension/wcrichton/flowistry). In VSCode: 
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

If you are interested in using the underlying analysis, take a look at the documentation for the `flowistry` crate: https://willcrichton.net/flowistry/flowistry/

## Usage

### 1. Startup

Once you have installed Flowistry, open a Rust workspace in VSCode. You should see this icon in the bottom toolbar:

<kbd>
<img width="121" alt="Screen Shot 2022-02-22 at 11 46 12 AM" src="https://user-images.githubusercontent.com/663326/155207447-4efb34b1-5e3d-4416-89aa-a0f7cd334ca4.png">
</kbd>
 <br /><br />

Flowistry starts up by type-checking your codebase. This may take a few minutes if you have many dependencies.

> Note: Flowistry type-checking results are cached in the `target/flowistry` directory. If you delete this folder, Flowistry will have to recompute types. Also for a large codebase this directory may take up a fair amount of disk space.

### 2. Entering focus mode

Once Flowistry has booted up, the loading icon will disappear. Then you can enter focus mode by running the "Toggle focus mode" command. By default the keyboard shortcut is Ctrl+R Ctrl+A (⌘+R ⌘+A on Mac), or you can use the Flowistry context menu:

<kbd>
<img width="450" alt="Screen Shot 2022-02-22 at 11 52 39 AM" src="https://user-images.githubusercontent.com/663326/155208449-83fceff7-86fe-4fe8-9fca-62552e6b0b43.png">
</kbd>
 <br /><br />

In focus mode, Flowistry will automatically compute the information flow within a given function once you put your cursor there. Once Flowistry has finished analysis, the status bar will look like this:

<kbd>
<img width="120" alt="Screen Shot 2022-02-22 at 11 55 36 AM" src="https://user-images.githubusercontent.com/663326/155208955-1f47ab0c-f3ae-4028-82b3-6f67e6fc0db7.png">
</kbd>
 <br /><br />
 
> Note: Flowistry can be a bit slow for larger functions. It may take up to 15 seconds to finish the analysis.

Flowistry infers what you want to focus on based on your cursor. So if you click on a variable, you should see the focus region of that variable. Flowistry will highlight the focused code in gray, and then fade out code outside the focus region. For example, because the user's cursor is on `view_projection`, that variable is highlighted in gray, and its focus region is shown.

<kbd>
<img width="900" alt="Screen Shot 2022-02-22 at 12 00 22 PM" src="https://user-images.githubusercontent.com/663326/155209805-75a23ed9-01ba-4100-b8bb-324b516f84cf.png">
</kbd>

### 3. Setting a mark

Sometimes you want to keep the focus region where it is, and click on other code to inspect it without changing focus. For this purpose, Flowistry has a concept of a "mark". Once you have selected code to focus on, you can run the "Set mark" command (Ctrl+R Ctrl+S / ⌘+R ⌘+S). Then a mark is set at your cursor's current position, and the focus will stay there until you run the "Unset mark" command (Ctrl+R Ctrl+D / ⌘+R ⌘+D).

### 4. Selecting the focus region

If you want to modify all the code in the focus region, e.g. to comment it out or copy it, then you can run the "Select focused region" command (Ctrl+R Ctrl+T / ⌘+R ⌘+T). This will add the entire focus region into your editor's selection.

## Limitations

Flowistry is an active research project into the applications of information flow analysis for Rust. It is continually evolving as we experiment with analysis techniques and interaction paradigms. So it's not quite as polished or efficient as tools like Rust Analyzer, but we hope you can still find it useful! Nevertheless, there are a number of important limitations you should understand when using Flowistry to avoid being surprised.

If you have questions or issues, please [file a Github issue](https://github.com/willcrichton/flowistry/issues), [join our Discord](https://discord.gg/XkcpkQn2Ah), or [DM @wcrichton on Twitter](https://twitter.com/wcrichton).

### 1. Flowistry does not completely handle interior mutability

When your code has references, Flowistry needs to understand what that reference points-to. Flowistry uses Rust's lifetime information to determine points-to information. However, data structures that use interior mutability such as `Arc<Mutex<T>>` explicitly *do not* share lifetimes between pointers to the same data. For example, in this snippet: 

```rust
let x = Arc::new(Mutex::new(0));
let y = x.clone();
*x.lock().unwrap() = 1;
println!("{}", y.lock().unwrap());
```

Flowistry *can* determine that `*x.lock().unwrap() = 1` is a mutation to `x`, but is *can not* determine that it is a mutation to `y`. So if you focus on `y`, the assignment to 1 would be faded out, even though it is relevant to the value of `y`.

We are researching methods to overcome this limitation, but for now just be aware that this is the main case where Flowistry is known to provide an incorrect answer.

### 2. A focus region may include more code than you expect

Flowistry's analysis tries to include all code that *could* have an influence on a focal point. This analysis makes a number of assumptions for both practical and fundamental reasons. For example, in this snippet:

```rust
let mut v = vec![1, 2, 3];
let x = v.get_mut(0);
println!("{:?} {}", v, x);
```

If you focus on `v` on line 3, it will include `v.get_mut(0)` as an operation that could have modified `v`. The reason is that Flowistry does not actually analyze the bodies of called functions, but rather approximates based on their type signatures. Because `get_mut` takes `&mut self` as input, it assumes that the vector *could* be modified.

In general, you should use focus mode as a pruning tool. If code is faded out, then you don't have to read it (minus the limitation mentioned above!). If it isn't faded out, then it might be relevant to your task.

### 3. Not all code is selectable

Flowistry works by analyzing the [MIR](https://rustc-dev-guide.rust-lang.org/mir/index.html) graph for a given function using the Rust compiler's API. Then the IDE extension lifts the analysis results from the MIR level back to the source level. However, a lot of information about the program is lost in the journey from source code to MIR. 

For example, if the source contains an expression `foo.whomp.bar().baz()`, it's possible that a temporary variable is only generated for the expression `foo.whomp.bar()`. So if the user selects `foo`, Flowistry may not be able to determine that this corresponds to the MIR [place](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/struct.Place.html) that represents `foo`.

This is why the IDE extension highlights the focused code in gray, so you can understand what your cursor's selection actually maps to.

## FAQ

### 1. rustup fails on installation

If rustup fails, especially with an error like "could not rename downloaded file", this is probably because Flowistry is running rustup concurrently with another tool (like rust-analyzer). Until [rustup#988](https://github.com/rust-lang/rustup/issues/988) is resolved, there is unfortunately no automated way around this. 

To solve the issue, go to the command line and run:

```
rustup toolchain install nightly-2022-02-17 -c rust-src -c rustc-dev -c llvm-tools-preview
```

> Note: double check the value of "channel" in `rust-toolchain.toml` if `nightly-2022-02-17` is no longer correct.

Then go back to VSCode and click "Continue" to let Flowistry continue installing.

### 2. Why isn't Flowistry part of Rust Analyzer?

Rust Analyzer does not support [MIR](https://rustc-dev-guide.rust-lang.org/mir/index.html) and the borrow checker, which are essential parts of Flowistry's analysis. That fact is unlikely to change for a [long time](https://rust-lang.zulipchat.com/#narrow/stream/185405-t-compiler.2Frust-analyzer/topic/How.20far.20is.20RA.20from.20MIR.3F), so Flowistry is a standalone tool.
