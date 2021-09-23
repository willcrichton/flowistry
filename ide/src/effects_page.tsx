import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom";
import {
  Message,
  ArgSlice,
  RetSlice,
  SelectedSlice,
  EffectStrings,
} from "./types";
import _ from "lodash";
import classNames from "classnames";

interface VSCode {
  postMessage(message: any): void;
  getState(): any;
  setState(state: any): void;
}

declare global {
  interface Window {
    acquireVsCodeApi: () => VSCode;
  }
}

const vscode = window.acquireVsCodeApi();

let Code: React.FC<{ children: string }> = ({ children }) => (
  <code>{children}</code>
  /* <Editor
    defaultLanguage="rust"
    defaultValue={children}
    height="1em"
    options={{
      renderLineHighlight: "none",
      minimap: { enabled: false },
      scrollbar: { vertical: "hidden" },
      lineNumbers: "off",
      readOnly: true
    }}
  /> */
);

let App: React.FC = () => {
  let [data, set_data] = useState<null | EffectStrings>(null);
  let [selected, set_selected] = useState<null | SelectedSlice>(null);
  useEffect(() => {
    window.addEventListener("message", (event) => {
      let message: Message = event.data;
      if (message.type === "input") {
        let effects: any = message.data;
        set_data(effects);
      }
    });
  }, []);
  return (
    <>
      <div>
        {data !== null ? (
          <div>
            {data.arg_strs.length == 0 && data.ret_strs.length == 0
              ? "This function has no effects!"
              : null}
            <ul>
              {data.arg_strs.map((arg_str, i) => (
                <li className="slice-category">
                  Arg: <Code>{arg_str.arg}</Code>
                  <ul>
                    {arg_str.effects.map((s, j) => {
                      s = s.replace(/[\n]/g, "");
                      s = s.replace(/^([\w\d_\s.]+)\(.*\)$/, "$1(..)");
                      let msg: ArgSlice = {
                        type: "arg",
                        arg_index: i,
                        effect_index: j,
                      };
                      return (
                        <li
                          className={classNames("slice-link", {
                            selected: _.isEqual(selected, msg),
                          })}
                          onClick={() => {
                            set_selected(msg);
                            vscode.postMessage({
                              type: "click",
                              data: msg,
                            });
                          }}
                        >
                          <Code>{s}</Code>
                        </li>
                      );
                    })}
                  </ul>
                </li>
              ))}
              {data.ret_strs.length > 0 ? (
                <li className="slice-category">
                  Returns
                  <ul>
                    {data.ret_strs.map((s, i) => {
                      let msg: RetSlice = { type: "ret", index: i };
                      return (
                        <li
                          className={classNames("slice-link", {
                            selected: _.isEqual(selected, msg),
                          })}
                          key={i}
                          onClick={() => {
                            set_selected(msg);
                            vscode.postMessage({
                              type: "click",
                              data: msg,
                            });
                          }}
                        >
                          <Code>{s}</Code>
                        </li>
                      );
                    })}
                  </ul>
                </li>
              ) : null}
            </ul>
          </div>
        ) : null}
      </div>
    </>
  );
};

ReactDOM.render(<App />, document.getElementById("app")!);
