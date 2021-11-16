import path from 'path';
import { runTests } from 'vscode-test';
import { MOCK_PROJECT_DIRECTORY, MOCK_PROJECT_FILES } from './unit/util/constants';

async function main() {
    // The folder containing the Extension Manifest package.json
    // Passed to `--extensionDevelopmentPath`
    const extensionDevelopmentPath = path.resolve(__dirname, '../../');

    const launchArgs = ["--disable-extensions", MOCK_PROJECT_DIRECTORY, ...Object.values(MOCK_PROJECT_FILES)];

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