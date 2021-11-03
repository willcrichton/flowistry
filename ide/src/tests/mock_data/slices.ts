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
  {
    "test": "basic_unused",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[7, 4], [7, 14]]
  },
  {
    "test": "basic_update",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[13, 4], [13, 18]]
  },
  {
    "test": "condition",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[19, 4], [19, 14]]
  },
  {
    "test": "pointer_write",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[30, 4], [30, 18]]
  },
  {
    "test": "function_params",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[35, 19], [35, 20]]
  },
  {
    "test": "struct_param",
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
    "slice_on": [[41, 16], [41, 17]]
  },
] as TestSlice[];
