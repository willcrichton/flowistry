import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom";
import { Message } from "./types";
import Editor from "@monaco-editor/react";

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
  let [data, set_data] = useState<null | any>(null);
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
      <style>{`
      code { color: black; font-family: Menlo, monospace; }
    `}</style>
      <div>
        <h1>Flowistry</h1>
        {data !== null ? (
          <div>
            <ul>
              {data.arg_strs.map((arg_str, i) => (
                <li>
                  Arg <Code>{arg_str.arg}</Code>
                  <ul>
                    {arg_str.effects.map((s, j) => (
                      <li
                        onClick={() => {
                          vscode.postMessage({
                            type: "click",
                            data: {
                              type: "arg",
                              arg_index: i,
                              effect_index: j,
                            },
                          });
                        }}
                      >
                        <Code>{s}</Code>
                      </li>
                    ))}
                  </ul>
                </li>
              ))}
              {data.ret_strs.length > 0 ? (
                <li>
                  Returns
                  <ul>
                    {data.ret_strs.map((s, i) => (
                      <li
                        onClick={() => {
                          vscode.postMessage({
                            type: "click",
                            data: { type: "ret", index: i },
                          });
                        }}
                      >
                        <Code>{s}</Code>
                      </li>
                    ))}
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
