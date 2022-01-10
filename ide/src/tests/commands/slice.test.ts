import { suite, test } from "mocha";
import { forward_slice_test_commands, backward_slice_test_commands } from "./mock_data/slice";
import { expect_commands } from "./util/helpers";

suite("Slice tests", async () => {
  test("forward select", expect_commands(forward_slice_test_commands));

  test("backward select", expect_commands(backward_slice_test_commands));
});
