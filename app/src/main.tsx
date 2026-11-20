import React from "react";
import ReactDOM from "react-dom/client";

import { App } from "./App";
import "./i18n";
import "./styles.css";
import { initContextMenu } from "./utils/contextMenu";
import { initTheme } from "./utils/theme";

initTheme();
initContextMenu();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
