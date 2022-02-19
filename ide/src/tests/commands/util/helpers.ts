import { expect } from "chai";
import _ from "lodash";
import vscode from "vscode";
import { exec_notify, flowistry_cmd, get_flowistry_opts } from "../../../setup";
import { CommandOutput } from "../../../types";
import { to_vsc_range } from "../../../range";
import { MOCK_PROJECT_DIRECTORY } from "../../constants";

export type TestCommand = {
    test: string;
    file: string;
    flowistry_subcmd: "focus";
    vscode_cmd: "focus";
    selection: [[number, number], [number, number]];
};

type TestCommandResult = {
    test: string;
    expected_selections: vscode.Selection[];
    actual_selections: vscode.Selection[];
};

/**
 * Returns the output of `cargo flowistry <command> <file> <selection>`.
 * @param param0 Title passed to a VSCode notification and arguments passed to `cargo flowistry`.
 * @returns Output from `cargo flowistry`.
 */
export const get_flowistry_output = async ({ test, file, flowistry_subcmd, selection }: TestCommand): Promise<string> => {
    const doc = vscode.window.activeTextEditor?.document!;
    const start = doc.offsetAt(new vscode.Position(...selection[0]));
    // const end = doc.offsetAt(new vscode.Position(...selection[1]));
    const command = `${flowistry_cmd} ${flowistry_subcmd} ${file} ${start}`;
    const command_opts = await get_flowistry_opts(MOCK_PROJECT_DIRECTORY);

    const output = await exec_notify(command, test, command_opts);

    return output;
};

/**
 * Highlights and performs a command on a range in a VSCode text document.
 * @param command VSCode extension command (eg. `forward_select`).
 * @param position Zero-based positions for the beginning and end of the range to perform the command on.
 * @param filename Filename of the VSCode text document containing the range to perform the command on.
 */
const perform_vscode_command = async (
    command: TestCommand['vscode_cmd'],
    position: TestCommand['selection'],
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

    await vscode.commands.executeCommand(`flowistry.${command}`);
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
 * Get the expected and actual selections after performing a command on a selection in a VSCode text document.
 * @param test_command Title for the command and arguments passed to the command.
 * @returns The command's expected selections, actual selections, and `test_command`.
 */
export const get_command_selections = async (test_command: TestCommand): Promise<TestCommandResult> => {
    await perform_vscode_command(test_command.vscode_cmd, test_command.selection, test_command.file);

    const raw_output_data = await get_flowistry_output(test_command);
    const output_data: CommandOutput = JSON.parse(raw_output_data).fields[0];

    const unique_ranges = _.uniqWith(output_data.ranges, _.isEqual);
    const sorted_ranges = _.sortBy(unique_ranges, (range) => [range.start]);
    const vscode_ranges = sorted_ranges.map((range) => to_vsc_range(range, vscode.window.activeTextEditor?.document!));
    const merged_ranges = merge_ranges(vscode_ranges);
    const expected_selections = merged_ranges.map((range) => new vscode.Selection(range.start, range.end));

    const actual_selections = vscode.window.activeTextEditor?.selections!;

    return {
        ...test_command,
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

/**
 * Assert VSCode commands highlight the expected Flowistry command output.
 * @param commands The VSCode/Flowistry commands to execute.
 */
export const expect_commands = (commands: TestCommand[]) => async () => {
    const selections = await resolve_sequentially(commands, get_command_selections);

    selections.forEach(async (selection) => {
      expect(selection.expected_selections).to.be.deep.equalInAnyOrder(selection.actual_selections);
    });
};
