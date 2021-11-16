import vscode from "vscode";
import { exec_notify } from "../../../setup";
import { TestSlice } from "../mock_data/slices";
import { MOCK_PROJECT_DIRECTORY } from "./constants";

export declare const TOOLCHAIN: {
    channel: string;
    components: string[];
};

const LIBRARY_PATHS: Partial<Record<NodeJS.Platform, string>> = {
    darwin: "DYLD_LIBRARY_PATH",
    win32: "LIB",
};

export const get_slice = async ({ test, file, direction, slice_on }: TestSlice): Promise<string> => {
    const doc = vscode.window.activeTextEditor?.document!;
    const start = doc.offsetAt(new vscode.Position(...slice_on[0]));
    const end = doc.offsetAt(new vscode.Position(...slice_on[1]));
    const flowistry_cmd = `cargo +${TOOLCHAIN.channel} flowistry`;
    const slice_command = `${flowistry_cmd} ${direction}_slice ${file} ${start} ${end}`;

    const rustc_path = await exec_notify(
        `rustup which --toolchain ${TOOLCHAIN.channel} rustc`,
        "Waiting for rustc..."
    );
    const target_info = await exec_notify(
        `${rustc_path} --print target-libdir --print sysroot`,
        "Waiting for rustc..."
    );
    const [target_libdir, sysroot] = target_info.split("\n");
    const library_path = LIBRARY_PATHS[process.platform] || "LD_LIBRARY_PATH";

    const output = await exec_notify(slice_command, test, {
        cwd: MOCK_PROJECT_DIRECTORY,
        [library_path]: target_libdir,
        SYSROOT: sysroot,
        RUST_BACKTRACE: "1",
    });

    return output;
};
