import { MOCK_PROJECT_FILES } from "../../constants";
import { TestCommand } from "../util/helpers";

export const focus_test_commands: TestCommand[] = [
  {
    "test": "constant",
    "selection": [[2, 4], [2, 5]],
    "file": MOCK_PROJECT_FILES.backward_slice,
    "flowistry_subcmd": "focus",
    "vscode_cmd": "focus",
  },
];
