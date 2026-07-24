import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { Overlay } from "./Overlay";
import { bootstrapDocument } from "./overlaySurface";
import "./styles.css";

bootstrapDocument(window.location.search, (root, overlay, generation) => {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      {overlay ? (
        generation ? (
          <Overlay kind={overlay} generation={generation} />
        ) : null
      ) : (
        <App />
      )}
    </React.StrictMode>
  );
});
