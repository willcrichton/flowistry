import { MOCK_PROJECT_FILES } from "../util/constants";

export type TestSlice = {
  test: string;
  file: string;
  direction: "forward" | "backward";
  slice_on: [[number, number], [number, number]];
};

export default [
  {
    "test": "constant",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[1, 16], [1, 17]]
  },
  {
    "test": "basic_variable",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[2, 12], [2, 13]]
  },
] as TestSlice[];
