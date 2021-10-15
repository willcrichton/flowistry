import assert from "assert";
import { suite, before, describe, it } from "mocha";
import _ from "lodash";
import vscode from "vscode";
import forward_slice from "../mock_data/forward_slice.json";

suite("Extension Test Suite", () => {
  const delay = (millis: number) =>
    new Promise<void>((resolve) => {
      setTimeout((_) => resolve(), millis);
    });

  before(async () => {
    vscode.window.showInformationMessage("Start all tests");
    await vscode.window.showTextDocument(vscode.workspace.textDocuments[0]);

    // Select 'x' on line 3 of forward_slice.rs
    const startXPos = new vscode.Position(2, 12);
    const endXPos = new vscode.Position(2, 13);
    vscode.window.activeTextEditor!.selection = new vscode.Selection(
      startXPos,
      endXPos
    );

    // Extensions commands aren't available for a couple seconds
    await delay(5000);
  });

  describe("Forward slice", () => {
    it("of constant highlights correct values", async () => {
      await vscode.commands.executeCommand("flowistry.forward_select");

      // Ugly workaround to get the values from the Selection class
      const actualSelection = JSON.parse(
        JSON.stringify(
          _.uniqWith(vscode.window.activeTextEditor?.selections, _.isEqual)
        )
      );
      const expectedSelection = forward_slice;

      assert.deepStrictEqual(expectedSelection, actualSelection);
    });
  });
});
