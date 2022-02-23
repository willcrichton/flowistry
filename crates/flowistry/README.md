# Information flow analysis

This crate contains the core analysis used by the [Flowistry](https://github.com/willcrichton/flowistry) IDE plugin. It is a Rust compiler plugin that computes the information flow within a function. The main analysis is at [`flowistry::infoflow::compute_flow`](https://github.com/willcrichton/flowistry/blob/master/crates/flowistry/src/infoflow/mod.rs).

Documentation: [https://willcrichton.net/flowistry/flowistry/](https://willcrichton.net/flowistry/flowistry/)

For details about this analysis, please read our paper ["Modular Information Flow Through Ownership"](https://arxiv.org/abs/2111.13662). If you use Flowistry in your research, please cite our paper:

```bibtex
@misc{crichton2021modular,
      title={Modular Information Flow Through Ownership}, 
      author={Will Crichton and Marco Patrignani and Maneesh Agrawala and Pat Hanrahan},
      year={2021},
      eprint={2111.13662},
      archivePrefix={arXiv},
      primaryClass={cs.PL}
}
```