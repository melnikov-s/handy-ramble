import React from "react";
import ReactDOM from "react-dom/client";
import { ClippingOverlay } from "./ClippingOverlay";
import "@/App.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ClippingOverlay />
  </React.StrictMode>,
);
