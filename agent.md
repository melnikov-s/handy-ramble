# Ramble - Ambient AI Interface

This program is a speech-to-text application. Its purpose is to provide a robust and user-friendly interface for transcribing spoken language. The application aims to offer features such as real-time transcription, post-processing options, and various settings for customization.

---

## Vision: The Ambient AI Layer

The goal is to create a **Jarvis-like intelligent interface** - an always-ready layer that sits between you and your OS. It's not a fully autonomous assistant that handles complex multi-step workflows; it's a **rapid interfacing tool** for quick, contextual interactions with AI.

### Core Philosophy

- **Voice is first-class** - Speaking should be the primary interaction mode
- **Context flows seamlessly** - Select text, take a screenshot, and it's instantly available to the agent
- **Access is immediate** - No switching apps or windows; the AI is _right there_
- **Bidirectional** - Push context from apps → agent, pull results from agent → apps

### The Problem Being Solved

The main bottleneck with AI agents today is **passing context**. You have to:

1. Copy text manually
2. Switch to a chat app
3. Paste and explain
4. Copy the response
5. Switch back and paste

Ramble removes this friction entirely.

---

## Feature Brainstorm

### 🎯 Inline Chat Window

A small, floating window that appears on demand:

- **Trigger**: Hotkey after selecting text, or voice command
- **Context-aware**: Automatically includes selected text, active window info, or screenshot
- **Ephemeral**: Quick back-and-forth, then dismiss - not a persistent chat history
- **Injection**: Results can be injected back into the originating app (paste, type)

**Open questions:**

- Should it remember context across multiple invocations in a session?
- How to handle multi-turn within a single popup session?

---

### 🎤 Voice-First Interactions

Building on existing transcription:

- **"Hey Ramble"** wake word (optional) for hands-free activation
- **Voice commands** that are context-aware: "Summarize this", "Fix the grammar", "Explain this code"
- **Conversational follow-ups**: "Now make it shorter", "Add bullet points"

---

### 📸 Context Capture Modes

| Mode       | Trigger   | What's Captured             |
| ---------- | --------- | --------------------------- |
| Selection  | Hotkey    | Selected text + app name    |
| Screenshot | Hotkey    | Screen region → OCR + image |
| Window     | Hotkey    | Active window content       |
| Clipboard  | Automatic | Whatever's in clipboard     |
| Voice      | Speaking  | Transcribed speech          |

All modes should **compose** - select text AND speak a question about it.

---

### 🔌 Output Actions

After the agent responds, what happens?

- **Paste**: Inject into current cursor position
- **Replace**: Replace selected text with response
- **Copy**: Just put in clipboard
- **Speak**: TTS reads the response
- **Preview**: Show in popup for review before action

---

### 🧠 Possible Future Directions

1. **Persistent Scratchpad** - A place for longer-form thinking, not just quick Q&A
2. **Workflow Triggers** - "When I say 'email this', draft an email with context"
3. **App Integrations** - Deep hooks into specific apps (browser, editor, terminal)
4. **Memory/Recall** - "What was that thing I asked about yesterday?"
5. **Sketching/Diagrams** - Voice-to-diagram, describe → generate

---

## What Ramble Is NOT

- Not a fully autonomous personal assistant
- Not a task manager or calendar
- Not something that "picks up kids from school"
- Not a replacement for dedicated IDE copilots

It's a **rapid context bridge** between your thoughts and AI.
