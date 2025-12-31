import React from "react";
import ReactDOM from "react-dom/client";
import { ChatWindow } from "@/components/chat/ChatWindow";
import "@/App.css";

// Error boundary class component to catch rendering errors
class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  { hasError: boolean; error: Error | null }
> {
  constructor(props: { children: React.ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("ErrorBoundary caught error:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{ padding: 20, color: "red", background: "#1a1a1a" }}>
          <h2>Something went wrong!</h2>
          <pre style={{ whiteSpace: "pre-wrap", fontSize: 12 }}>
            {this.state.error?.message}
            {"\n\n"}
            {this.state.error?.stack}
          </pre>
        </div>
      );
    }
    return this.props.children;
  }
}

// Get initial context and messages from window URL params if provided
const urlParams = new URLSearchParams(window.location.search);
const initialContext = urlParams.get("context") || undefined;

// Parse initial messages for forked conversations
let initialMessages: Array<{ role: string; content: string }> | undefined;
const messagesParam = urlParams.get("messages");
if (messagesParam) {
  try {
    // URLSearchParams.get() already decodes the URL parameter, so no need for decodeURIComponent
    console.log(
      "main.tsx: Parsing messages parameter",
      messagesParam.length,
      "chars",
    );
    initialMessages = JSON.parse(messagesParam);
    console.log(
      "main.tsx: Successfully parsed",
      initialMessages?.length,
      "messages",
    );
  } catch (e) {
    console.error("main.tsx: Failed to parse initial messages parameter:", e);
    console.error("Raw parameter preview:", messagesParam.substring(0, 100));
  }
}

console.log("main.tsx: Rendering ChatWindow with:", {
  hasContext: !!initialContext,
  messagesCount: initialMessages?.length || 0,
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <ChatWindow
        initialContext={initialContext}
        initialMessages={initialMessages}
      />
    </ErrorBoundary>
  </React.StrictMode>,
);
