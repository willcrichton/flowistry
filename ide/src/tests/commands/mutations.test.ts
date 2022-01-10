import { suite, test } from "mocha";
import { mutation_test_commands } from "./mock_data/mutations";
import { expect_commands } from "./util/helpers";

suite("Mutation tests", async () => {
  test("find mutations", expect_commands(mutation_test_commands));
});
