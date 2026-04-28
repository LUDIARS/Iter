import React from "react";
import ReactDOM from "react-dom/client";
import { FileWindow } from "./FileWindow";
import "./styles.css";

// File ウインドウは ?path=... をクエリで受け取る。
const params = new URLSearchParams(window.location.search);
const path = params.get("path") ?? "";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <FileWindow path={path} />
  </React.StrictMode>,
);
