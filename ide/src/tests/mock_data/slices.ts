import { MOCK_PROJECT_FILES } from "../util/constants";

export type TestSlice = {
  test: string;
  file: string;
  direction: "forward" | "backward";
  slice_on: [[number, number], [number, number]];
};

const forward_slices = [
  {
    "test": "constant",
    "slice_on": [[1, 16], [1, 17]],
  },
].map((slice) => ({
  "file": MOCK_PROJECT_FILES.forward_slice,
  "direction": "forward",
  ...slice,
}));

const backward_slices = [
  {
    "test": "variable_read",
    "slice_on": [[2, 4], [2, 5]],
  },
].map((slice) => ({
  "file": MOCK_PROJECT_FILES.backward_slice,
  "direction": "backward",
  ...slice,
}));

export default [...forward_slices, ...backward_slices] as TestSlice[];
