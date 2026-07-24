import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { native } from "./native";
import { Overlay } from "./Overlay";
import { bootstrapDocument } from "./overlaySurface";
import "./styles.css";

bootstrapDocument(
  window.location.search,
  (root, overlay) => {
    ReactDOM.createRoot(root).render(
      <React.StrictMode>
        {overlay ? <Overlay kind={overlay} /> : <App />}
      </React.StrictMode>
    );
  },
  (overlay) => {
    void native.overlaySurfaceReady(overlay).catch(() => undefined);
  }
);
