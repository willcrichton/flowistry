// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as util from 'util';
import * as path from 'path';

const exec = util.promisify(cp.exec);

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
export async function activate(context: vscode.ExtensionContext) {
	log('rust-slicer is activated');

	let {stdout} = await exec('$(rustup which --toolchain nightly rustc) --print sysroot --print target-libdir');
	let [sysroot, lib_dir] = stdout.trim().split('\n');
	log(sysroot, lib_dir, stdout.trim());

	let decoration_type = vscode.window.createTextEditorDecorationType({
		backgroundColor: new vscode.ThemeColor('editor.findMatchHighlightBackground')
	});

	let active_editor = vscode.window.activeTextEditor;

	let folders = vscode.workspace.workspaceFolders;
	if (!folders || folders.length == 0) { return; }
	let workspace_root = folders[0].uri.fsPath;

	// The command has been defined in the package.json file
	// Now provide the implementation of the command with registerCommand
	// The commandId parameter must match the command field in package.json
	let disposable = vscode.commands.registerCommand('rust-slicer.slice', async () => {
		if (!active_editor) { return; }

		let file_path = active_editor.document.fileName;
		file_path = path.relative(workspace_root, file_path);
		
		let selection = active_editor.selection;

		let env = {...process.env, DYLD_LIBRARY_PATH: lib_dir, LD_LIBRARY_PATH: lib_dir};
		let cmd = `rust-slicer-cli ${file_path} ${sysroot} ${selection.start.line} ${selection.start.character} ${selection.end.line} ${selection.end.character}`;

		try {
			log("Running command:", cmd);
			let {stdout} = await exec(cmd, {env, cwd: workspace_root});

			let lines = stdout.trim().split("\n");
			let last_line = lines[lines.length - 1];
			let slice_output: SliceOutput = JSON.parse(last_line);

			let decorations = slice_output.ranges.map(slice_range => {
				let range = new vscode.Range(slice_range.start_line, slice_range.start_col,
					slice_range.end_line, slice_range.end_col);
				return {range};
			});

			active_editor.setDecorations(decoration_type, decorations)

			let callback = vscode.workspace.onDidChangeTextDocument(event => {
				if (!active_editor) { return; }
				if (event.document != active_editor.document) { return; }
				active_editor.setDecorations(decoration_type, []);
				callback.dispose();
			})
		} catch (exc) {
			log("ERROR", exc);
		}
	});

	context.subscriptions.push(disposable);
}

// this method is called when your extension is deactivated
export function deactivate() { }
