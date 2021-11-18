import path from 'path';
import { runTests } from 'vscode-test';
import minimist from 'minimist';
import { Mockttp } from 'mockttp';
import { MOCK_PROJECT_DIRECTORY, MOCK_PROJECT_FILES } from './unit/util/constants';
import { createProxy, PROXY_PORT } from './install/proxy';

async function main() {
    const args = minimist(process.argv.slice(2));
    const isInstall = args._.includes('install');
    const zip = args['zip'];
    let server: Mockttp | undefined;

    // The folder containing the Extension Manifest package.json
    // Passed to `--extensionDevelopmentPath`
    const extensionDevelopmentPath = path.resolve(__dirname, '../../');

    const launchArgs = ["--disable-extensions", MOCK_PROJECT_DIRECTORY, ...Object.values(MOCK_PROJECT_FILES)];

    if (isInstall && zip) {
        server = await createProxy(zip);
        launchArgs.push(`--proxy-server=http://localhost:${PROXY_PORT}`, '--ignore-certificate-errors')
    }

    // All test suites (either unit tests or integration tests) should be in subfolders.
    const unitTestsPath = path.resolve(__dirname, './unit/index');
    const installTestsPath = path.resolve(__dirname, './install/index');

    // Run tests using the latest stable release of VSCode
    await runTests({
        version: 'stable',
        launchArgs,
        extensionDevelopmentPath,
        extensionTestsPath: isInstall ? installTestsPath : unitTestsPath,
    });

    server?.stop();
}

main().catch(err => {
    console.error('Failed to run tests', err);
    process.exit(1);
});
