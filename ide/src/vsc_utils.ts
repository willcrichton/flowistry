import * as vscode from "vscode";
import { Range } from "./types";

let channel = vscode.window.createOutputChannel("Flowistry");
export let log = (...strs: any[]) => {
  channel.appendLine(strs.map((obj) => String(obj)).join("\t"));
};

export let to_vsc_range = (
  range: Range,
  doc: vscode.TextDocument
): vscode.Range =>
  new vscode.Range(doc.positionAt(range.start), doc.positionAt(range.end));

export let from_vsc_range = (range: vscode.Range, doc: vscode.TextDocument): Range => ({
  start: doc.offsetAt(range.start),
  end: doc.offsetAt(range.end),
  filename: "",
});

export let show_error = (err: string) => {
  vscode.window.showErrorMessage(`Flowistry error: ${err}`);
};

export type CallFlowistry = (args: string) => Promise<string>;
