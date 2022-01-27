import { MOCK_PROJECT_FILES } from "../../constants";
import { TestCommand } from "../util/helpers";

export const mutation_test_commands: TestCommand[] = [
  {
    "test": "basic reassign",
    "selection": [[1, 8], [1, 13]],
    "file": MOCK_PROJECT_FILES.find_mutations,
    "flowistry_subcmd": "find_mutations",
    "vscode_cmd": "select_mutations",
  },
];
