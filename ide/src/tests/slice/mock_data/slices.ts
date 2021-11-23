import { MOCK_PROJECT_FILES } from "../../constants";

export type TestSlice = {
  test: string;
  file: string;
  direction: "forward" | "backward";
  slice_on: [[number, number], [number, number]];
};

export const forward_slices: TestSlice[] = [
  {
    "test": "constant",
    "slice_on": [[1, 16], [1, 17]],
    "file": MOCK_PROJECT_FILES.forward_slice,
    "direction": "forward",
  },
];

export const backward_slices: TestSlice[] = [
  {
    "test": "variable_read",
    "slice_on": [[2, 4], [2, 5]],
    "file": MOCK_PROJECT_FILES.backward_slice,
    "direction": "backward",
  },
];
