# Information flow analysis

This crate contains the core analysis used by the [Flowistry](https://github.com/willcrichton/flowistry) IDE plugin. It is a Rust compiler plugin that computes the information flow within a function. The main analysis is at [`flowistry::infoflow::compute_flow`](https://github.com/willcrichton/flowistry/blob/master/crates/flowistry/src/infoflow/mod.rs).

Documentation: [https://willcrichton.net/flowistry/flowistry/](https://willcrichton.net/flowistry/flowistry/)

For details about this analysis, please read our paper ["Modular Information Flow Through Ownership"](https://arxiv.org/abs/2111.13662). If you use Flowistry in your research, please cite our paper:

```bibtex
@inproceedings{crichton2022,
  author = {Crichton, Will and Patrignani, Marco and Agrawala, Maneesh and Hanrahan, Pat},
  title = {Modular Information Flow through Ownership}, year = {2022},
  isbn = {9781450392655}, publisher = {Association for Computing Machinery},
  address = {New York, NY, USA}, url = {https://doi.org/10.1145/3519939.3523445},
  booktitle = {Proceedings of the 43rd ACM SIGPLAN International Conference on Programming Language Design and Implementation},
  pages = {1–14}, numpages = {14}, keywords = {information flow, rust, ownership types},
  location = {San Diego, CA, USA}, series = {PLDI 2022}, doi = {10.1145/3519939.3523445},
}
```