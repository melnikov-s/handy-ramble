import React from "react";
import ReactDOM from "react-dom/client";
import { ChatWindow } from "@/components/chat/ChatWindow";
import "@/App.css";

// Get initial context from window URL params if provided
const urlParams = new URLSearchParams(window.location.search);
const initialContext = urlParams.get("context") || undefined;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ChatWindow initialContext={initialContext} />
  </React.StrictMode>,
);
