import assert from 'assert';
import { before } from 'mocha';
import vscode from 'vscode';

suite('Extension Test Suite', () => {
    before(() => {
        vscode.window.showInformationMessage('Start all tests');
    });

    test('Placeholder test', () => {
        assert.strictEqual(true, true);
    })
})