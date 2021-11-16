import chai, { expect } from "chai";
import deepEqualAnyOrder from 'deep-equal-in-any-order';
import { suite, before, describe, it } from "mocha";
import _ from "lodash";
import vscode from "vscode";
import slices, { TestSlice } from "./mock_data/slices";
import { get_slice } from "./util/get_slice";
import { SliceOutput } from "../../types";
import { to_vsc_range } from "../../vsc_utils";

const slice = async (
  direction: TestSlice['direction'],
  position: TestSlice['slice_on'],
  filename: string,
): Promise<void> => {
  const file = vscode.Uri.parse(filename);
  await vscode.window.showTextDocument(file);

  const start_position = new vscode.Position(...position[0]);
  const end_position = new vscode.Position(...position[1]);

  vscode.window.activeTextEditor!.selection = new vscode.Selection(
    start_position,
    end_position
  );

  await vscode.commands.executeCommand(`flowistry.${direction}_select`);
};

const merge_ranges = (ranges: vscode.Range[]): vscode.Range[] => {
  const merged_ranges = [ranges[0]];

  ranges.slice(1).forEach((range) => {
    const last_range = merged_ranges[merged_ranges.length - 1];
    const intersection = last_range.intersection(range);

    if (!intersection) {
      merged_ranges.push(range);
    }
    else {
      const union = last_range.union(range);
      merged_ranges[merged_ranges.length - 1] = union;
    }
  });

  return merged_ranges;
};

type TestSliceResult = {
  test: string;
  expected_selections: vscode.Selection[];
  actual_selections: vscode.Selection[];
};

const get_slice_selections = async (test_slice: TestSlice): Promise<TestSliceResult> => {
  await slice(test_slice.direction, test_slice.slice_on, test_slice.file);
  
  const raw_slice_data = await get_slice(test_slice);
  const slice_data: SliceOutput = JSON.parse(raw_slice_data).fields[0];

  const unique_ranges = _.uniqWith(slice_data.ranges, _.isEqual);
  const sorted_ranges = _.sortBy(unique_ranges, (range) => [range.start]);
  const vscode_ranges = sorted_ranges.map((range) => to_vsc_range(range, vscode.window.activeTextEditor?.document!));
  const merged_ranges = merge_ranges(vscode_ranges);
  const expected_selections = merged_ranges.map((range) => new vscode.Selection(range.start, range.end));

  const actual_selections = vscode.window.activeTextEditor?.selections!;

  return {
    ...test_slice,
    expected_selections,
    actual_selections,
  };
};

const resolve_sequentially = async <T, R>(items: T[], resolver: (arg0: T) => Promise<R>): Promise<R[]> => {
  const results: R[] = [];

  for (const item of items) {
    const result = await resolver(item);
    results.push(result);
  }

  return results;
};

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
