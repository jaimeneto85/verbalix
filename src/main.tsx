import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { Overlay } from "./Overlay";
import "./styles.css";

const overlay = new URLSearchParams(window.location.search).get("overlay");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {overlay === "toolbar" || overlay === "note" ? (
      <Overlay kind={overlay} />
    ) : (
      <App />
    )}
  </React.StrictMode>
);
