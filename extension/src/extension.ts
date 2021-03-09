// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';
import * as cp from 'child_process';

const SLICER_PATH = '/Users/will/Code/rust-slicer/target/debug/rust-slicer';
let channel = vscode.window.createOutputChannel("Rust Slicer");
let log = (...strs: any[]) => {
	channel.appendLine(strs.map(obj => String(obj)).join('\t'));
};

interface SliceRange {
	start_line: number,
	start_col: number,
	end_line: number,
	end_col: number
}

interface SliceOutput {
	ranges: SliceRange[]
}

// this method is called when your extension is activated
// your extension is activated the very first time the command is executed
export function activate(context: vscode.ExtensionContext) {
	log('rust-slicer is activated');

	let decoration_type = vscode.window.createTextEditorDecorationType({
		backgroundColor: new vscode.ThemeColor('editor.findMatchHighlightBackground')
	});

	let active_editor = vscode.window.activeTextEditor;

	// The command has been defined in the package.json file
	// Now provide the implementation of the command with registerCommand
	// The commandId parameter must match the command field in package.json
	let disposable = vscode.commands.registerCommand('rust-slicer.slice', () => {
		if (!active_editor) { return; }

		let file_path = active_editor.document.fileName;
		let selection = active_editor.selection;
		let cmd = `DYLD_LIBRARY_PATH=/Users/will/Code/rust/build/x86_64-apple-darwin/stage1/lib/rustlib/x86_64-apple-darwin/lib/ \
			${SLICER_PATH} ${file_path} ${selection.start.line} ${selection.start.character} ${selection.end.line} ${selection.end.character}`;
		log("Running command:", cmd);
		cp.exec(cmd, (err, stdout, stderr) => {
			if (!active_editor) { return; }

			try {
				if (err != null) {
					throw stderr;
				} else {
					let lines = stdout.trim().split("\n");
					let last_line = lines[lines.length - 1];
					let slice_output: SliceOutput = JSON.parse(last_line);

					let decorations = slice_output.ranges.map(slice_range => {
						let range = new vscode.Range(slice_range.start_line, slice_range.start_col,
							slice_range.end_line, slice_range.end_col);
						return {range};
					});
					active_editor.setDecorations(decoration_type, decorations)
				}
			} catch (exc) {
				log("ERROR", exc);
			}
		});
	});

	context.subscriptions.push(disposable);
}

// this method is called when your extension is deactivated
export function deactivate() { }
