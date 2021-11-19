import chai, { expect } from "chai";
import deepEqualAnyOrder from 'deep-equal-in-any-order';
import { suite, before, test } from "mocha";
import _ from "lodash";
import { forward_slices, backward_slices } from "./mock_data/slices";
import { get_slice_selections, resolve_sequentially } from "./util/slice_helpers";

suite("Slice selection tests", async () => {
  before(() => {
    chai.use(deepEqualAnyOrder);
  });

  test("forward select", async () => {
    const forward_selections = await resolve_sequentially(forward_slices, get_slice_selections);

    forward_selections.forEach(async (selection) => {
      expect(selection.expected_selections).to.be.deep.equalInAnyOrder(selection.actual_selections);
    });
  });

  test("backward select", async () => {
    const backward_selections = await resolve_sequentially(backward_slices, get_slice_selections);

    backward_selections.forEach(async (selection) => {
      expect(selection.expected_selections).to.be.deep.equalInAnyOrder(selection.actual_selections);
    });
  });
});
