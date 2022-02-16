import vscode from "vscode";

export type FocusStatus =
  | "active"
  | "unsaved"
  | "inactive"
  | "error"
  | "loading";

type FocusStatusBarConfig = {
  foreground: string;
  background: string;
  icon?: string;
  command: string;
};

const focus_configs: Record<FocusStatus, FocusStatusBarConfig> = {
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
  inactive: {
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
};

export let render_status_bar = (
  item: vscode.StatusBarItem,
  state: FocusStatus,
  tooltip?: string
) => {
  let config = focus_configs[state];

  item.color = config.foreground;
  item.backgroundColor = new vscode.ThemeColor(config.background);
  item.text = `$(${config.icon}) focus mode`;
  item.command = config.command;
  item.tooltip = tooltip;
};
