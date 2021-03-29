// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as util from 'util';
import * as path from 'path';

const TOOLCHAIN = 'local';

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

	let {stdout} = await exec(`$(rustup which --toolchain ${TOOLCHAIN} rustc) --print target-libdir`);
	let lib_dir = stdout.trim();
  log("Rustc target-libdir", lib_dir);

	let decoration_type = vscode.window.createTextEditorDecorationType({
		backgroundColor: new vscode.ThemeColor('editor.findMatchHighlightBackground')
	});


	let folders = vscode.workspace.workspaceFolders;
	if (!folders || folders.length == 0) { return; }
	let workspace_root = folders[0].uri.fsPath;
  log("Workspace root", workspace_root);

	let slice =  async (standard: boolean) => {
	  let active_editor = vscode.window.activeTextEditor;
		if (!active_editor) { return; }

		let file_path = active_editor.document.fileName;
		let selection = active_editor.selection;

		let env = {...process.env, DYLD_LIBRARY_PATH: lib_dir, LD_LIBRARY_PATH: lib_dir};
		let mode = standard ? '' : '-l';
		let cmd = `rust-slicer-cli ${mode} ${file_path} ${selection.start.line} ${selection.start.character} ${selection.end.line} ${selection.end.character} `;

		try {
			log("Running command:");
			log(cmd);

			let {stdout} = await exec(cmd, {env, cwd: workspace_root});

			let lines = stdout.trim().split("\n");
			let last_line = lines[lines.length - 1];
			let slice_output: SliceOutput = JSON.parse(last_line);


			let decorations = slice_output.ranges.map(slice_range => {
				let range = new vscode.Range(slice_range.start_line, slice_range.start_col,
					slice_range.end_line, slice_range.end_col);
				return {range};
			});

			if (decorations.length == 0) {
				let selected_text = active_editor.document.getText(selection);
				vscode.window.showInformationMessage(`Slice on "${selected_text}" did not generate any results`);
				return;
			}

			active_editor.setDecorations(decoration_type, decorations)

			let callback = vscode.workspace.onDidChangeTextDocument(event => {
				if (!active_editor) { return; }
				if (event.document != active_editor.document) { return; }
				active_editor.setDecorations(decoration_type, []);
				callback.dispose();
			})
		} catch (exc) {
			log("ERROR", exc);
			vscode.window.showErrorMessage(`Rust Slicer failed with error: ${exc}`);
		}
	};

	// The command has been defined in the package.json file
	// Now provide the implementation of the command with registerCommand
	// The commandId parameter must match the command field in package.json
	let disposable = vscode.commands.registerCommand('rust-slicer.slice', () => slice(true));
	context.subscriptions.push(disposable);

	disposable = vscode.commands.registerCommand('rust-slicer.slice_c', () => slice(false));
	context.subscriptions.push(disposable);
}

export function deactivate() { }
