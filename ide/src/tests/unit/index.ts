import path from 'path';
import Mocha from 'mocha';
import glob from 'glob-promise';

export async function run(): Promise<void> {
    // Create the mocha test
    const mocha = new Mocha({
        ui: 'tdd',
        color: true
    });

    const testsRoot = __dirname;

    const files = await glob('**/**.test.js', { cwd: testsRoot });

    return new Promise((resolve, reject) => {
        // Add files to the test suite
        files.map(f => path.resolve(testsRoot, f)).forEach(mocha.addFile);

        try {
            // Run the mocha test
            mocha.timeout(10000);
            mocha.run(failures => {
                if (failures > 0) {
                    reject(new Error(`${failures} tests failed.`));
                } else {
                    resolve();
                }
            });
        } catch (err) {
            reject(err);
        }
    });
}
