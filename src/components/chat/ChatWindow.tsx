import React, { useEffect, useState, useRef } from "react";
import {
  AssistantRuntimeProvider,
  useLocalRuntime,
  type ChatModelAdapter,
} from "@assistant-ui/react";
import { Thread } from "./Thread";
import { commands, LLMModel, LLMProvider, ChatMessage } from "@/bindings";
import {
  XIcon,
  ChevronDownIcon,
  Loader2Icon,
  CopyIcon,
  CheckIcon,
} from "lucide-react";
import { cn } from "@/lib/utils/cn";
import { useThreadRuntime, useThread } from "@assistant-ui/react";
import { useChatPersistence } from "../../hooks/useChatPersistence";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Component to copy entire chat as markdown (must be inside AssistantRuntimeProvider)
const CopyAllHeader: React.FC = () => {
  const [copied, setCopied] = React.useState(false);
  const thread = useThread();

  const handleCopyAll = async () => {
    const messages = thread.messages;
    if (messages.length === 0) return;

    // Format messages as markdown
    const markdown = messages
      .map((msg) => {
        const role = msg.role === "user" ? "**User:**" : "**Assistant:**";
        const content = msg.content
          .filter(
            (part): part is { type: "text"; text: string } =>
              part.type === "text",
          )
          .map((part) => part.text)
          .join("");
        return `${role}\n\n${content}`;
      })
      .join("\n\n---\n\n");

    try {
      await navigator.clipboard.writeText(markdown);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (error) {
      console.error("Failed to copy:", error);
    }
  };

  // Don't show if no messages
  if (thread.messages.length === 0) return null;

  return (
    <div className="sticky top-0 z-10 flex justify-end px-4 py-1">
      <button
        onClick={handleCopyAll}
        className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs text-[var(--color-text)]/50 hover:bg-[var(--color-text)]/10 hover:text-[var(--color-text)] transition-colors"
        title="Copy entire conversation as markdown"
      >
        {copied ? (
          <>
            <CheckIcon className="h-3.5 w-3.5 text-green-500" />
            <span className="text-green-500">Copied!</span>
          </>
        ) : (
          <>
            <CopyIcon className="h-3.5 w-3.5" />
            <span>Copy All</span>
          </>
        )}
      </button>
    </div>
  );
};

interface ChatWindowProps {
  initialContext?: string;
  initialMessages?: Array<{ role: string; content: string }>;
  onClose?: () => void;
}

// Component to load initial messages into the thread (for forked conversations)
const InitialMessageLoader: React.FC<{
  messages?: Array<{ role: string; content: string }>;
}> = ({ messages }) => {
  const runtime = useThreadRuntime();
  const [loaded, setLoaded] = React.useState(false);

  useEffect(() => {
    if (messages && messages.length > 0 && !loaded && runtime) {
      try {
        console.log(
          "InitialMessageLoader: Importing",
          messages.length,
          "messages",
        );
        // Import messages into the thread using ExportedMessageRepository format
        // Build messages with proper parent chain and required metadata
        const exportedMessages = messages.map((msg, index) => ({
          message: {
            id: `forked-${index}`,
            role: (msg.role === "user" ? "user" : "assistant") as
              | "user"
              | "assistant",
            content: [{ type: "text" as const, text: msg.content || "" }],
            createdAt: new Date(),
            status: { type: "complete" as const },
            // Add metadata with unstable_state to satisfy @assistant-ui/react
            metadata: {},
          },
          parentId: index === 0 ? null : `forked-${index - 1}`,
        }));

        // Wait a frame to ensure runtime is fully initialized
        const timer = setTimeout(() => {
          runtime.import({ messages: exportedMessages } as any);
          setLoaded(true);
        }, 0);

        return () => clearTimeout(timer);
      } catch (err) {
        console.error("InitialMessageLoader: Failed to import messages:", err);
      }
    }
  }, [messages, runtime, loaded]);

  return null;
};

// Remove internal createChatAdapter as it's now handled inside the stable runtime state

import { ModelsDropdown } from "../ui/ModelsDropdown";

// Remove internal ModelSelector component as it's replaced by shared ModelsDropdown

export const ChatWindow: React.FC<ChatWindowProps> = ({
  initialContext,
  initialMessages,
  onClose,
}) => {
  const [models, setModels] = useState<LLMModel[]>([]);
  const [providers, setProviders] = useState<LLMProvider[]>([]);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [currentInitialMessages, setCurrentInitialMessages] = useState<
    Array<{ role: string; content: string }> | undefined
  >(initialMessages);
  const [isLoading, setIsLoading] = useState(true);
  const [initialPrompt, setInitialPrompt] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);
  const [chatIdState, setChatIdState] = useState<number | null>(null);
  const { saveChat, updateChat, getChat, generateTitle, updateChatTitle } =
    useChatPersistence();

  // Load models, providers and saved chat on mount
  useEffect(() => {
    const loadData = async () => {
      try {
        // Check for chatId in URL
        const urlParams = new URLSearchParams(window.location.search);
        const urlChatId = urlParams.get("chatId");
        let messagesToLoad = initialMessages;

        if (urlChatId) {
          const id = parseInt(urlChatId);
          if (!isNaN(id)) {
            const savedChat = await getChat(id);
            if (savedChat) {
              setChatIdState(id);
              const msgs = savedChat.messages.map((msg) => ({
                role: msg.role,
                content: msg.content,
              }));
              setCurrentInitialMessages(msgs);
              console.log("ChatWindow: Loaded saved chat", id);
              // Set window title to the saved chat's title
              if (savedChat.title && savedChat.title !== "New Chat") {
                getCurrentWindow().setTitle(savedChat.title);
              }
            }
          }
        }
        const [
          modelsResult,
          providersResult,
          defaultModelsResult,
          settingsResult,
        ] = await Promise.all([
          commands.getLlmModels(),
          commands.getLlmProviders(),
          commands.getDefaultModels(),
          commands.getAppSettings(),
        ]);

        // Only show enabled models in the dropdown
        const enabledModels = modelsResult.filter((m) => m.enabled);
        setModels(enabledModels);
        setProviders(providersResult);

        if (settingsResult.status === "ok") {
          setInitialPrompt(settingsResult.data.quick_chat_initial_prompt || "");
        }

        // Use default chat model if available and enabled
        if (defaultModelsResult.chat) {
          const isEnabled = enabledModels.some(
            (m) => m.id === defaultModelsResult.chat,
          );
          if (isEnabled) {
            setSelectedModelId(defaultModelsResult.chat);
          } else if (enabledModels.length > 0) {
            setSelectedModelId(enabledModels[0].id);
          }
        } else if (enabledModels.length > 0) {
          setSelectedModelId(enabledModels[0].id);
        }
      } catch (error) {
        console.error("Failed to load models:", error);
      } finally {
        setIsLoading(false);
      }
    };

    loadData();

    // Poll for pending clips instead of using events (events are unreliable across windows)
    const pollInterval = setInterval(async () => {
      try {
        const pendingClip = await commands.getPendingClip();
        if (pendingClip) {
          console.log(
            "ChatWindow: Got pending clip",
            pendingClip.length,
            "chars",
          );
          setAttachments((prev) => [...prev, pendingClip]);
        }
      } catch (err) {
        // Ignore errors during polling
      }
    }, 500);

    return () => {
      clearInterval(pollInterval);
    };
  }, []);

  // Stabilize adapter and accessories with refs
  const attachmentsRef = useRef(attachments);
  const selectedModelIdRef = useRef(selectedModelId);
  const initialPromptRef = useRef(initialPrompt);
  const initialContextRef = useRef(initialContext);
  const chatIdStateRef = useRef(chatIdState);

  useEffect(() => {
    attachmentsRef.current = attachments;
    selectedModelIdRef.current = selectedModelId;
    initialPromptRef.current = initialPrompt;
    initialContextRef.current = initialContext;
    chatIdStateRef.current = chatIdState;
  }, [
    attachments,
    selectedModelId,
    initialPrompt,
    initialContext,
    chatIdState,
  ]);

  const [runtime] = useState(() => {
    const adapter: ChatModelAdapter = {
      async *run({ messages }) {
        const allMessages: ChatMessage[] = [];
        let processedPrompt = initialPromptRef.current;
        if (initialContextRef.current) {
          processedPrompt = processedPrompt.replace(
            "${selection}",
            initialContextRef.current,
          );
        } else {
          processedPrompt = processedPrompt.replace(
            "${selection}",
            "[No selection provided]",
          );
        }

        allMessages.push({
          role: "system",
          content: processedPrompt,
          images: null,
        });

        const formattedMessages = [
          ...allMessages,
          ...messages.map((msg, index) => ({
            role: msg.role,
            content:
              typeof msg.content === "string"
                ? msg.content
                : msg.content
                    .filter((part) => part.type === "text")
                    .map(
                      (part) => (part as { type: "text"; text: string }).text,
                    )
                    .join(""),
            images:
              msg.role === "user" && index === messages.length - 1
                ? attachmentsRef.current
                : null,
          })),
        ];

        try {
          const response = await commands.chatCompletion(
            formattedMessages as any,
            selectedModelIdRef.current,
            true, // Always enable grounding (backend handles provider filtering)
          );

          if (response.status === "ok") {
            setAttachments([]);
            const assistantContent = response.data.content;

            // Auto-save logic
            setTimeout(async () => {
              try {
                const currentMessages = formattedMessages.map((msg) => ({
                  role: msg.role,
                  content: msg.content,
                  images: msg.images || null,
                }));

                // Add the new assistant message
                currentMessages.push({
                  role: "assistant",
                  content: assistantContent,
                  images: null,
                });

                if (chatIdStateRef.current) {
                  await updateChat(chatIdStateRef.current, currentMessages);
                } else {
                  // New chat - save and generate title
                  const newId = await saveChat(currentMessages, "New Chat");
                  setChatIdState(newId);

                  // Generate title after first user msg + assistant resp
                  // Only if this is the FIRST exchange (messages count will be 3: system, user, assistant)
                  if (currentMessages.length <= 3) {
                    const userMsg = messages[messages.length - 1];
                    const userText =
                      typeof userMsg.content === "string"
                        ? userMsg.content
                        : userMsg.content
                            .filter((p) => p.type === "text")
                            .map((p) => (p as any).text)
                            .join("");

                    try {
                      const title = await generateTitle(
                        userText,
                        assistantContent,
                      );
                      if (title) {
                        await updateChatTitle(newId, title);
                        // Update window title to match the chat title
                        getCurrentWindow().setTitle(title);
                      }
                    } catch (titleErr) {
                      console.error("Failed to generate title:", titleErr);
                    }
                  }
                }
              } catch (saveErr) {
                console.error("Failed to auto-save chat:", saveErr);
              }
            }, 0);

            yield {
              content: [{ type: "text" as const, text: assistantContent }],
              metadata: {
                custom: {
                  groundingMetadata: response.data.grounding_metadata,
                },
              },
            };
          } else {
            yield {
              content: [
                { type: "text" as const, text: `Error: ${response.error}` },
              ],
            };
          }
        } catch (error) {
          console.error("Chat completion error:", error);
          yield {
            content: [
              {
                type: "text" as const,
                text: `Error: ${error instanceof Error ? error.message : "Unknown error"}`,
              },
            ],
          };
        }
      },
    };
    return adapter;
  });

  const chatRuntime = useLocalRuntime(runtime as any);

  return (
    <div className="flex h-full flex-col bg-[var(--color-background)]">
      {/* Context indicator */}
      {initialContext && (
        <div className="border-b border-[var(--color-text)]/10 bg-[var(--color-text)]/5 px-4 py-2">
          <p className="text-xs text-[var(--color-text)]/70">
            Context from selection:
          </p>
          <p className="mt-1 line-clamp-2 text-sm text-[var(--color-text)]">
            {initialContext}
          </p>
        </div>
      )}

      {/* Chat thread */}
      <div className="relative flex-1 overflow-hidden bg-app-base">
        <AssistantRuntimeProvider runtime={chatRuntime}>
          <InitialMessageLoader messages={currentInitialMessages} />
          <CopyAllHeader />
          <Thread
            attachments={attachments}
            setAttachments={setAttachments}
            selectedModelId={selectedModelId}
            setSelectedModelId={setSelectedModelId}
            models={models}
            isLoading={isLoading}
          />
        </AssistantRuntimeProvider>
      </div>
    </div>
  );
};

export default ChatWindow;
