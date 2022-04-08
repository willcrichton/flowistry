## Benching process

The main benchmark file is `flowistry/benches/main.rs`. We use macros defined in the `bench_utils` crate to generate the code on which Flowistry is benchmarked. Each benchmark (information flow, locations, places, etc) is split into four distinct sub-benchmarks:

1. flow (min)
    - **Only** run `compute_flow` in the analysis; analyze a "minimum" function (small number of places, locations, etc)
2. flow (max)
    - **Only** run `compute_flow` in the analysis; analyze a "maximum" function (large number of places, locations, etc)
3. flow + deps (min)
    - Run `compute_flow` **and** `compute_dependencies` on the first place in the function; analyze a "minimum" function (small number of places, locations, etc)
4. flow + deps (max)
    - Run `compute_flow` **and** `compute_dependencies` on the first place in the function; analyze a "maximum" function (large number of places, locations, etc)

The numbers used for the "min/max" are currently quasi-arbitrary. The maximum acts as the ceiling (the benchmark will time out or take annoyingly long with larger values) and the minimum will be a fraction of the max. The same numbers are used for the benchmarks in each category (location-generating and place-generating) for an easier comparison across axes (places, locations, lifetimes).

## Adding a benchmark

The `TESTS` array in `flowistry/benches/main.rs` defines the programs that are benchmarked. To add a new benchmark, add a file for the "min" and "max" stresses in `flowistry/benches/tests/<max & min>/<new benchmark>` and add the filename to the `TESTS` array.

If you need to create a new macro to generate programs, add it to `bench_utils/src/lib.rs`.
