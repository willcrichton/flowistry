import chai, { expect } from "chai";
import deepEqualAnyOrder from 'deep-equal-in-any-order';
import { suite, before, describe, it } from "mocha";
import _ from "lodash";
import slices from "./mock_data/slices";
import { get_slice_selections, resolve_sequentially } from "./util/slice_helpers";

suite("Extension Test Suite", async () => {
  before(async function () {
    chai.use(deepEqualAnyOrder);

    // Run slices synchronously to avoid overlapping selections 
    const slice_test_cases = await resolve_sequentially(slices, get_slice_selections);

    ["forward", "backward"].forEach((direction) => {
      describe(`${direction} select`, function () {
        _.filter(slice_test_cases, ['direction', direction]).forEach((test_case) => {
          it(test_case.test, () => {
            expect(test_case.expected_selections).to.be.deep.equalInAnyOrder(test_case.actual_selections);
          });
        });
      });
    });
  });

  it('This is a required placeholder to allow before() to work');
});
