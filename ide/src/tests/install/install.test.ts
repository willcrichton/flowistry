import { suite, it } from "mocha";
import { expect } from "chai";
import vscode from "vscode";
import waitForExpect from "wait-for-expect";

const flowistryCommandsExist = async () => {
    const commands = await vscode.commands.getCommands();
    return commands.filter((command) => command.includes('flowistry.')).length > 0;
};

suite("Flowistry installation tests", () => {
    const timeout = 50 * 1000;

    it("Installs Flowistry", async () => {
        const interval = 1 * 1000;

        // Wait for Flowistry commands to exist, polling every second for 50 seconds
        await waitForExpect(async () => {
            expect(await flowistryCommandsExist()).to.be.true;
        }, timeout, interval);
    }).timeout(timeout);
});
