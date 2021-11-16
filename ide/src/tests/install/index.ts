import { ExTester } from "vscode-extension-tester";

export const tester = new ExTester();

const setup = async () => {
    await tester.runTests('./out/tests/install/*.test.js');
};

setup();
