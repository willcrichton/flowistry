import vscode from "vscode";

let channel = vscode.window.createOutputChannel("Flowistry");
export let logs: string[] = [];
export let log = (...strs: any[]) => {
  let s = strs.map((obj) => String(obj)).join("\t");
  logs.push(s);
  channel.appendLine(s);
  console.log(...strs);
};
