# Information flow analysis

This crate contains the core analysis used by the [Flowistry](https://github.com/willcrichton/flowistry) IDE plugin. It is a Rust compiler plugin that computes the information flow within a function. The main analysis is at [`flowistry::infoflow::compute_flow`](https://github.com/willcrichton/flowistry/blob/master/crates/flowistry/src/infoflow/mod.rs).

For details about this analysis, please read our paper ["Modular Information Flow Through Ownership"](COMING_SOON). If you use Flowistry in your research, please cite our paper:

```bibtex
@citation_coming_soon{}
```