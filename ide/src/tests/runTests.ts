import path from 'path';
import { runTests } from 'vscode-test';

async function main() {
    // The folder containing the Extension Manifest package.json
    // Passed to `--extensionDevelopmentPath`
    const extensionDevelopmentPath = path.resolve(__dirname, '../../');

    const launchArgs = ["--disable-extensions"];

    // All test suites (either unit tests or integration tests) should be in subfolders.
    const extensionTestsPath = path.resolve(__dirname, './unit/index');

    // Run tests using the latest stable release of VSCode
    await runTests({
        version: 'stable',
        launchArgs,
        extensionDevelopmentPath,
        extensionTestsPath
    });
}

main().catch(err => {
    console.error('Failed to run tests', err);
    process.exit(1);
});