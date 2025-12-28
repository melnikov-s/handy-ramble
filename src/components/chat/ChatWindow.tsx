import React, { useEffect, useState } from "react";
import {
  AssistantRuntimeProvider,
  useLocalRuntime,
  type ChatModelAdapter,
} from "@assistant-ui/react";
import { Thread } from "./Thread";
import { commands, LLMModel, LLMProvider } from "@/bindings";
import { XIcon, ChevronDownIcon, Loader2Icon } from "lucide-react";

interface ChatWindowProps {
  initialContext?: string;
  onClose?: () => void;
}

const createChatAdapter = (
  selectedModelId: string | null,
  initialPrompt: string,
  initialContext: string | undefined,
): ChatModelAdapter => {
  return {
    async *run({ messages }) {
      // Build starting messages with system prompt
      const allMessages = [];

      // Process initial prompt by replacing ${selection} if present
      let processedPrompt = initialPrompt;
      if (initialContext) {
        processedPrompt = processedPrompt.replace(
          "${selection}",
          initialContext,
        );
      } else {
        // If no context, remove most things related to selection
        processedPrompt = processedPrompt.replace(
          "${selection}",
          "[No selection provided]",
        );
      }

      // Add the processed system prompt
      allMessages.push({
        role: "system",
        content: processedPrompt,
      });

      // Convert messages to our backend format and append
      const formattedMessages = [
        ...allMessages,
        ...messages.map((msg) => ({
          role: msg.role,
          content:
            typeof msg.content === "string"
              ? msg.content
              : msg.content
                  .filter((part) => part.type === "text")
                  .map((part) => (part as { type: "text"; text: string }).text)
                  .join(""),
        })),
      ];

      try {
        // Call our Tauri backend for LLM response
        const response = await commands.chatCompletion(
          formattedMessages,
          selectedModelId,
        );

        if (response.status === "ok") {
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
};

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
  }, []);

  // Create adapter with current model selection
  const [adapter, setAdapter] = useState(() =>
    createChatAdapter(selectedModelId, initialPrompt, initialContext),
  );

  // Update adapter when model or initial prompt changes
  useEffect(() => {
    setAdapter(
      createChatAdapter(selectedModelId, initialPrompt, initialContext),
    );
  }, [selectedModelId, initialPrompt, initialContext]);

  const runtime = useLocalRuntime(adapter);

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
      <div className="flex-1 overflow-hidden">
        <AssistantRuntimeProvider runtime={runtime}>
          <Thread />
        </AssistantRuntimeProvider>
      </div>
    </div>
  );
};

export default ChatWindow;
