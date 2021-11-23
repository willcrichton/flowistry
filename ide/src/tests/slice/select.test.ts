import chai, { expect } from "chai";
import deepEqualAnyOrder from 'deep-equal-in-any-order';
import { suite, before, test } from "mocha";
import _ from "lodash";
import { forward_slices, backward_slices, TestSlice } from "./mock_data/slices";
import { get_slice_selections, resolve_sequentially } from "./util/slice_helpers";

const slice_test = (slices: TestSlice[]) => async () => {
  const selections = await resolve_sequentially(slices, get_slice_selections);

  selections.forEach(async (selection) => {
    expect(selection.expected_selections).to.be.deep.equalInAnyOrder(selection.actual_selections);
  });
}

suite("Slice selection tests", async () => {
  before(() => {
    chai.use(deepEqualAnyOrder);
  });

  test("forward select", slice_test(forward_slices));

  test("backward select", slice_test(backward_slices));
});
