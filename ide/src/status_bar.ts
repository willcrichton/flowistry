import vscode from "vscode";

export type StatusBarState =
  | "active"
  | "unsaved"
  | "idle"
  | "error"
  | "loading"
  | "notfound";

interface StatusBarConfig {
  foreground: string;
  background: string;
  icon?: string;
  command: string;
  tooltip?: string;
}

const config_for_state: Record<StatusBarState, StatusBarConfig> = {
  active: {
    foreground: "statusBarItem.warningForeground",
    background: "statusBarItem.warningBackground",
    icon: "check",
    command: "flowistry.focus",
  },
  unsaved: {
    foreground: "statusBarItem.foreground",
    background: "statusBarItem.background",
    icon: "circle-slash",
    command: "flowistry.focus",
  },
  idle: {
    foreground: "statusBarItem.foreground",
    background: "statusBarItem.background",
    command: "flowistry.focus",
  },
  error: {
    foreground: "statusBarItem.errorForeground",
    background: "statusBarItem.errorBackground",
    icon: "x",
    command: "flowistry.last_error",
  },
  loading: {
    foreground: "statusBarItem.foreground",
    background: "statusBarItem.background",
    icon: "sync~spin",
    command: "flowistry.focus",
  },
  notfound: {
    foreground: "statusBarItem.foreground",
    background: "statusBarItem.background",
    icon: "question",
    command: "flowistry.focus",
    tooltip: "Flowistry could not get Cargo to find this file (this is probably a Flowistry bug)"
  },
};

export class StatusBar {
  bar_item: vscode.StatusBarItem;
  state: StatusBarState = "loading";

  constructor(context: vscode.ExtensionContext) {
    this.bar_item = vscode.window.createStatusBarItem();
    context.subscriptions.push(this.bar_item);
    this.bar_item.show();
  }

  set_state(state: StatusBarState, tooltip: string = "") {
    this.state = state;
    this.bar_item.tooltip = tooltip;
    this.render();
  }

  render() {
    let config = config_for_state[this.state];
    this.bar_item.color = config.foreground;
    this.bar_item.backgroundColor = new vscode.ThemeColor(config.background);
    this.bar_item.text = `$(${config.icon}) flowistry`;
    this.bar_item.command = config.command;
    this.bar_item.tooltip = config.tooltip;
  }
}
