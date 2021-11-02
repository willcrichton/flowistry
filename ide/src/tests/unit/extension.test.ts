import chai, { expect } from "chai";
import deepEqualAnyOrder from 'deep-equal-in-any-order';
import { suite, before, describe, it } from "mocha";
import _ from "lodash";
import vscode from "vscode";
import forward_slice from "../mock_data/forward_slice.json";
import { MOCK_PROJECT_FILES } from "../util/constants";

const slice = async (
  direction: 'forward' | 'backward',
  position: [[number, number], [number, number]],
  filename: string,
): Promise<void> => {
  const file = vscode.Uri.parse(filename);
  await vscode.window.showTextDocument(file);

  const start_position = new vscode.Position(position[0][0], position[0][1]);
  const end_position = new vscode.Position(position[1][0], position[1][1]);

  vscode.window.activeTextEditor!.selection = new vscode.Selection(
    start_position,
    end_position
  );

  await vscode.commands.executeCommand(`flowistry.${direction}_select`);
}

suite("Extension Test Suite", () => {
  before(async () => {
    chai.use(deepEqualAnyOrder);
  });

  describe("Forward slice", () => {
    it("of constant highlights correct values", async () => {
      await slice("forward", [[2, 12], [2, 13]], MOCK_PROJECT_FILES.forward_slice);

      // Ugly workaround to get the values from the Selection class
      const actualSelection = JSON.parse(
        JSON.stringify(
          _.uniqWith(vscode.window.activeTextEditor?.selections, _.isEqual)
        )
      );
      const expectedSelection = forward_slice;

      expect(actualSelection).to.deep.equalInAnyOrder(expectedSelection);
    });
  });
});
