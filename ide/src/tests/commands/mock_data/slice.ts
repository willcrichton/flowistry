import { MOCK_PROJECT_FILES } from "../../constants";
import { TestCommand } from "../util/helpers";

export const forward_slice_test_commands: TestCommand[] = [
  {
    "test": "constant",
    "selection": [[1, 16], [1, 17]],
    "file": MOCK_PROJECT_FILES.forward_slice,
    "flowistry_subcmd": "forward_slice",
    "vscode_cmd": "forward_select",
  },
];

export const backward_slice_test_commands: TestCommand[] = [
  {
    "test": "variable_read",
    "selection": [[2, 4], [2, 5]],
    "file": MOCK_PROJECT_FILES.backward_slice,
    "flowistry_subcmd": "backward_slice",
    "vscode_cmd": "backward_select",
  },
];
