import vscode from 'vscode';

export enum FocusStatusBarState {
    Active,
    Inactive,
    Error
};

type FocusStatusBarConfig = {
    foreground: string,
    background: string,
    icon: string,
    command: string,
};

const focus_configs: Record<FocusStatusBarState, FocusStatusBarConfig> = {
    [FocusStatusBarState.Active]: {
        foreground: 'statusBarItem.warningForeground',
        background: 'statusBarItem.warningBackground',
        icon: 'check',
        command: 'flowistry.focus',

    },
    [FocusStatusBarState.Inactive]: {
        foreground: 'statusBarItem.foreground',
        background: 'statusBarItem.background',
        icon: 'circle-slash',
        command: 'flowistry.focus',
    },
    [FocusStatusBarState.Error]: {
        foreground: 'statusBarItem.errorForeground',
        background: 'statusBarItem.errorBackground',
        icon: 'x',
        command: 'flowistry.last_err',
    },
};

export let render_status_bar = (item: vscode.StatusBarItem, state: FocusStatusBarState) => {
    let config = focus_configs[state];

    item.color = config.foreground;
    item.backgroundColor = new vscode.ThemeColor(config.background);
    item.text = `$(${config.icon}) Flowistry: focus mode`;
    item.command = config.command;
};
