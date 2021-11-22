import _ from "lodash";
import vscode from "vscode";
import { exec_notify, flowistry_cmd, get_flowistry_opts } from "../../../setup";
import { SliceOutput } from "../../../types";
import { to_vsc_range } from "../../../vsc_utils";
import { TestSlice } from "../mock_data/slices";
import { MOCK_PROJECT_DIRECTORY } from "../../constants";

type TestSliceResult = {
    test: string;
    expected_selections: vscode.Selection[];
    actual_selections: vscode.Selection[];
};

/**
 * Returns the output of `cargo flowistry <direction>_slice <file> <slice information>`.
 * @param param0 Title passed to a VSCode notification and arguments passed to `cargo flowistry`.
 * @returns Output from `cargo flowistry`.
 */
export const get_slice = async ({ test, file, direction, slice_on }: TestSlice): Promise<string> => {
    const doc = vscode.window.activeTextEditor?.document!;
    const start = doc.offsetAt(new vscode.Position(...slice_on[0]));
    const end = doc.offsetAt(new vscode.Position(...slice_on[1]));
    const slice_command = `${flowistry_cmd} ${direction}_slice ${file} ${start} ${end}`;
    const command_opts = await get_flowistry_opts(MOCK_PROJECT_DIRECTORY);

    const output = await exec_notify(slice_command, test, command_opts);

    return output;
};

/**
 * Highlights and performs a slice on a range in a VSCode text document.
 * @param direction Slice direction.
 * @param position Zero-based positions for the beginning and end of the range to slice on.
 * @param filename Filename of the VSCode text document containing the range to slice on.
 */
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

/**
 * Merge overlapping VSCode Ranges.
 * @param ranges Array of ranges to merge.
 * @returns Array of merged ranges.
 */
const merge_ranges = (ranges: vscode.Range[]): vscode.Range[] => {
    const merged_ranges = [ranges[0]];

    ranges.slice(1).forEach((range) => {
        const last_range = _.last(merged_ranges)!;
        const intersection = last_range.intersection(range);

        // If the current and previous ranges have no overlap
        if (!intersection) {
            // Add the current range to `merged_ranges`
            merged_ranges.push(range);
        }
        else {
            // Set the previous range to the union of the current and previous ranges
            const union = last_range.union(range);
            merged_ranges[merged_ranges.length - 1] = union;
        }
    });

    return merged_ranges;
};

/**
 * Get the expected and actual selections of a slice on a value in a VSCode text document.
 * @param test_slice Title for the slice and arguments passed to the slice command.
 * @returns The slice's expected selections, actual selections, and `test_slice`.
 */
export const get_slice_selections = async (test_slice: TestSlice): Promise<TestSliceResult> => {
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

/**
 * Sequentially maps an array of values using a Promise-like `resolver` function.
 * @param items Array of items to pass to `resolver`.
 * @param resolver Function which resolves a new value from an element of `items`.
 * @returns A new array of `items` where each element is the result of `await resolver(<element>)`.
 */
export const resolve_sequentially = async <T, R>(items: T[], resolver: (arg0: T) => PromiseLike<R>): Promise<R[]> => {
    const results: R[] = [];

    for (const item of items) {
        const result = await resolver(item);
        results.push(result);
    }

    return results;
};
