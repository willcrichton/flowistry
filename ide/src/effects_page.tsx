import React, {useEffect, useState} from "react";
import ReactDOM from "react-dom";
import {Effects, Message} from "./types";

interface VSCode {
  postMessage(message: any): void;
  getState(): any;
  setState(state: any): void;
}

declare global {
  interface Window { acquireVsCodeApi: () => VSCode }
}

const vscode = window.acquireVsCodeApi();

let App: React.FC = () => {
  let [text, set_text] = useState("");
  useEffect(() => {
    window.addEventListener('message', event => {
      let message: Message = event.data;
      if (message.type === "input") {
        let effects: Effects = message.data;
        set_text(JSON.stringify(effects));
        vscode.postMessage({
          type: "test",
          data: {ok: "HI"}
        });
      }
    });
  }, []);
  return <>Hello world: {text}</>;
}

ReactDOM.render(<App />, document.getElementById('app')!);