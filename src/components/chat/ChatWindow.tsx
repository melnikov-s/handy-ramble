import React, { useEffect, useState, useRef } from "react";
import {
  AssistantRuntimeProvider,
  useLocalRuntime,
  type ChatModelAdapter,
} from "@assistant-ui/react";
import { Thread } from "./Thread";
import { commands, LLMModel, LLMProvider, ChatMessage } from "@/bindings";
import { XIcon, ChevronDownIcon, Loader2Icon } from "lucide-react";

interface ChatWindowProps {
  initialContext?: string;
  onClose?: () => void;
}

// Remove internal createChatAdapter as it's now handled inside the stable runtime state

import { ModelsDropdown } from "../ui/ModelsDropdown";

// Remove internal ModelSelector component as it's replaced by shared ModelsDropdown

export const ChatWindow: React.FC<ChatWindowProps> = ({
  initialContext,
  onClose,
}) => {
  const [models, setModels] = useState<LLMModel[]>([]);
  const [providers, setProviders] = useState<LLMProvider[]>([]);
  const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [initialPrompt, setInitialPrompt] = useState("");
  const [attachments, setAttachments] = useState<string[]>([]);

  // Load models and providers on mount
  useEffect(() => {
    const loadData = async () => {
      try {
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

  useEffect(() => {
    attachmentsRef.current = attachments;
    selectedModelIdRef.current = selectedModelId;
    initialPromptRef.current = initialPrompt;
    initialContextRef.current = initialContext;
  }, [attachments, selectedModelId, initialPrompt, initialContext]);

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
          );

          if (response.status === "ok") {
            setAttachments([]);
            yield {
              content: [{ type: "text" as const, text: response.data }],
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
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[var(--color-text)]/10 px-4 py-2">
        <h2 className="text-sm font-medium text-[var(--color-text)]">
          Ramble Chat
        </h2>

        <div className="flex items-center gap-2">
          {/* Model selector */}
          {isLoading ? (
            <Loader2Icon className="h-4 w-4 animate-spin text-[var(--color-text)]/50" />
          ) : (
            <ModelsDropdown
              selectedValue={selectedModelId}
              onSelect={setSelectedModelId}
              className="w-auto"
            />
          )}

          {onClose && (
            <button
              onClick={onClose}
              className="rounded p-1 text-[var(--color-text)]/50 hover:bg-[var(--color-text)]/10 hover:text-[var(--color-text)]"
            >
              <XIcon className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>

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
      <div className="flex h-screen flex-col overflow-hidden bg-app-base">
        <AssistantRuntimeProvider runtime={chatRuntime}>
          <Thread attachments={attachments} setAttachments={setAttachments} />
        </AssistantRuntimeProvider>
      </div>
    </div>
  );
};

export default ChatWindow;
