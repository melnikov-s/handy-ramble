import {
  ActionBarPrimitive,
  BranchPickerPrimitive,
  ComposerPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
  useMessage,
  useThread,
} from "@assistant-ui/react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { GroundingMetadata } from "@/bindings";
import { useState } from "react";
import {
  ArrowUpIcon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  CopyIcon,
  RefreshCwIcon,
  XIcon,
  SearchIcon,
  ChevronDownIcon,
  Loader2Icon,
  GitForkIcon,
} from "lucide-react";
import React, { useEffect } from "react";
import type { FC } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { cn } from "@/lib/utils/cn";
import { Button } from "@/components/ui/Button";

import { ModelsDropdown } from "@/components/ui/ModelsDropdown";

interface ThreadProps {
  attachments: string[];
  setAttachments: React.Dispatch<React.SetStateAction<string[]>>;
  selectedModelId: string | null;
  setSelectedModelId: (id: string | null) => void;
  models: any[];
  isLoading: boolean;
}

export const Thread: FC<ThreadProps> = ({
  attachments,
  setAttachments,
  selectedModelId,
  setSelectedModelId,
  models,
  isLoading,
}) => {
  return (
    <ThreadPrimitive.Root className="aui-root flex h-full flex-col bg-[var(--color-background)]">
      <ThreadPrimitive.Viewport className="flex flex-1 flex-col overflow-y-auto scroll-smooth px-4 pt-4 pb-24">
        <ThreadPrimitive.Empty>
          <ThreadWelcome />
        </ThreadPrimitive.Empty>

        <ThreadPrimitive.Messages
          components={{
            UserMessage,
            AssistantMessage,
          }}
        />
      </ThreadPrimitive.Viewport>

      <div className="absolute bottom-0 left-0 right-0 mx-auto w-full max-w-2xl px-4 pb-4 bg-gradient-to-t from-[var(--color-background)] via-[var(--color-background)] to-transparent pt-8">
        <Composer
          attachments={attachments}
          setAttachments={setAttachments}
          selectedModelId={selectedModelId}
          setSelectedModelId={setSelectedModelId}
          models={models}
          isLoading={isLoading}
        />
      </div>
    </ThreadPrimitive.Root>
  );
};

const ThreadWelcome: FC = () => {
  return (
    <div className="flex h-full flex-col items-center justify-center">
      <div className="text-center">
        <h1 className="text-xl font-semibold text-[var(--color-text)]">
          Ramble Chat
        </h1>
        <p className="mt-2 text-sm text-[var(--color-text)]/70">
          Ask me anything about your selection or context
        </p>
      </div>
    </div>
  );
};

import { CameraIcon, CropIcon } from "lucide-react";
import { commands } from "@/bindings";

const Composer: FC<ThreadProps> = ({
  attachments,
  setAttachments,
  selectedModelId,
  setSelectedModelId,
  models,
  isLoading,
}) => {
  console.log("Composer rendering with attachments:", attachments.length);

  // Reinforce focus on mount
  useEffect(() => {
    const timer = setTimeout(() => {
      const input = document.querySelector(".chat-input") as
        | HTMLTextAreaElement
        | HTMLInputElement;
      if (input) {
        input.focus();
      }
    }, 100);
    return () => clearTimeout(timer);
  }, []);

  const handleScreenshot = async () => {
    try {
      const result = await commands.captureScreenMode(false);
      if (result.status === "ok") {
        setAttachments((prev) => [...prev, result.data]);
      }
    } catch (error) {
      console.error("Failed to capture screenshot:", error);
    }
  };

  const handleClip = async () => {
    try {
      await commands.openClippingTool();
    } catch (error) {
      console.error("Failed to open clipping tool:", error);
    }
  };

  const removeAttachment = (index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index));
  };

  return (
    <div className="flex w-full flex-col gap-2">
      {/* Attachment Previews */}
      {attachments.length > 0 && (
        <div className="flex flex-wrap gap-2 rounded-lg border border-[var(--color-text)]/10 bg-[var(--color-text)]/5 p-2">
          {attachments.map((base64, index) => (
            <div
              key={index}
              className="relative h-16 w-16 overflow-hidden rounded border border-[var(--color-text)]/20 shadow-sm"
            >
              <img
                src={`data:image/png;base64,${base64}`}
                alt="Attachment"
                className="h-full w-full object-cover"
              />
              <button
                onClick={() => removeAttachment(index)}
                className="absolute right-0.5 top-0.5 rounded-full bg-black/50 p-0.5 text-white hover:bg-black/70"
              >
                <XIcon className="h-3 w-3" />
              </button>
            </div>
          ))}
        </div>
      )}

      <ComposerPrimitive.Root className="flex w-full flex-col gap-2 rounded-xl border border-[var(--color-text)]/20 bg-[var(--color-background)] p-3 shadow-sm focus-within:border-[var(--color-logo-primary)]/50 transition-colors">
        <ComposerPrimitive.Input
          placeholder="Ask a question about the screen..."
          className="min-h-[60px] w-full resize-none bg-transparent text-sm text-[var(--color-text)] placeholder:text-[var(--color-text)]/40 focus:outline-none chat-input"
          autoFocus
        />

        <div className="flex items-center justify-between border-t border-[var(--color-text)]/10 pt-2 mt-auto">
          <div className="flex items-center gap-1">
            <button
              onClick={handleScreenshot}
              className="rounded p-2 text-[var(--color-text)]/50 hover:bg-[var(--color-text)]/10 hover:text-[var(--color-text)] transition-colors"
              title="Attach Screenshot"
            >
              <CameraIcon className="h-4 w-4" />
            </button>
            <button
              onClick={handleClip}
              className="rounded p-2 text-[var(--color-text)]/50 hover:bg-[var(--color-text)]/10 hover:text-[var(--color-text)] transition-colors"
              title="Clip Region"
            >
              <CropIcon className="h-4 w-4" />
            </button>

            <div className="mx-2 h-4 w-[1px] bg-[var(--color-text)]/10" />

            {/* Model selector integrated into the bottom bar */}
            {isLoading ? (
              <Loader2Icon className="h-4 w-4 animate-spin text-[var(--color-text)]/30" />
            ) : (
              <ModelsDropdown
                selectedValue={selectedModelId}
                onSelect={setSelectedModelId}
                className="w-auto border-none bg-transparent hover:bg-[var(--color-text)]/5 text-xs h-8"
                direction="up"
              />
            )}
          </div>

          <ComposerPrimitive.Send asChild>
            <Button
              variant="primary"
              size="sm"
              className="h-8 w-8 rounded-lg p-0 shadow-sm"
            >
              <ArrowUpIcon className="h-4 w-4" />
            </Button>
          </ComposerPrimitive.Send>
        </div>
      </ComposerPrimitive.Root>
    </div>
  );
};

const UserMessage: FC = () => {
  return (
    <MessagePrimitive.Root className="mb-4 flex justify-end">
      <div className="max-w-[80%] rounded-lg bg-[var(--color-logo-primary)] px-4 py-2 text-white">
        <MessagePrimitive.Content />
      </div>
    </MessagePrimitive.Root>
  );
};

const AssistantMessage: FC = () => {
  const message = useMessage();
  // @ts-ignore - types might be slightly off with custom metadata
  const groundingMetadata = message.metadata?.custom?.groundingMetadata as
    | GroundingMetadata
    | undefined;

  return (
    <MessagePrimitive.Root className="mb-4 flex flex-col group">
      <div className="w-fit max-w-[80%]">
        <div className="rounded-lg bg-[var(--color-text)]/10 px-4 py-2 text-[var(--color-text)] [&_p]:my-1 [&_ul]:list-disc [&_ul]:pl-4 [&_ol]:list-decimal [&_ol]:pl-4 [&_code]:bg-black/10 [&_code]:rounded [&_code]:px-1 [&_pre]:bg-black/10 [&_pre]:p-2 [&_pre]:rounded [&_pre]:overflow-x-auto">
          <MessagePrimitive.Content
            components={{
              Text: ({ text }) => (
                <Markdown remarkPlugins={[remarkGfm]}>{text}</Markdown>
              ),
            }}
          />
          {/* Loading indicator inside the message bubble if it's currently running */}
          {message.status?.type === "running" && (
            <div className="flex items-center gap-2 py-1 italic opacity-50">
              <Loader2Icon className="h-3 w-3 animate-spin" />
              <span className="text-xs">Thinking...</span>
            </div>
          )}
        </div>
        <div className="mt-1 flex items-center justify-between min-h-[32px] opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto transition-opacity duration-200">
          <div className="flex-1">
            <AssistantActionBar />
          </div>
          {groundingMetadata && (
            <div className="flex-shrink-0">
              <SearchGrounding metadata={groundingMetadata} />
            </div>
          )}
        </div>
      </div>
    </MessagePrimitive.Root>
  );
};

const SearchGrounding: FC<{ metadata: GroundingMetadata }> = ({ metadata }) => {
  const [isOpen, setIsOpen] = useState(false);

  if (!metadata.chunks || metadata.chunks.length === 0) return null;

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1.5 rounded-full border border-[var(--color-text)]/10 px-2 py-0.5 text-xs text-[var(--color-text)]/60 hover:bg-[var(--color-text)]/5 hover:text-[var(--color-text)]"
      >
        <SearchIcon className="h-3 w-3" />
        <span>Search Activated</span>
        <ChevronDownIcon
          className={cn("h-3 w-3 transition-transform", isOpen && "rotate-180")}
        />
      </button>
      {isOpen && (
        <div className="absolute bottom-full right-0 mb-2 z-50 flex max-h-[300px] w-80 flex-col gap-1.5 overflow-y-auto rounded-lg border border-[var(--color-text)]/20 bg-[var(--color-background)] p-3 shadow-xl">
          <div className="mb-2 flex items-center justify-between border-b border-[var(--color-text)]/10 pb-1">
            <span className="text-xs font-semibold">Search Sources</span>
            <button onClick={() => setIsOpen(false)}>
              <XIcon className="h-3 w-3" />
            </button>
          </div>
          {metadata.search_entry_point &&
            metadata.search_entry_point.includes("google-search-chip") && (
              <div
                className="mb-1 overflow-x-auto pb-2 [&_a]:text-[var(--color-primary)] [&_a]:hover:underline [&_.google-search-chip]:whitespace-nowrap [&_.google-search-chip]:inline-block"
                dangerouslySetInnerHTML={{
                  __html: metadata.search_entry_point,
                }}
              />
            )}
          {metadata.chunks.map((chunk, i) => {
            let displayUrl = "";
            let actualLink = chunk.uri || "#";
            let isVertexRedirect = false;

            try {
              if (chunk.uri) {
                const url = chunk.uri.toLowerCase();
                // Check if it's a Vertex AI or Google grounding redirect
                if (
                  url.includes("vertex") ||
                  url.includes("google.com/grounding")
                ) {
                  isVertexRedirect = true;
                  const urlObj = new URL(chunk.uri);
                  const extractedUrl =
                    urlObj.searchParams.get("url") ||
                    urlObj.searchParams.get("uri");
                  if (extractedUrl) {
                    actualLink = extractedUrl;
                    try {
                      const targetUrlObj = new URL(extractedUrl);
                      displayUrl = targetUrlObj.hostname;
                    } catch {
                      displayUrl = extractedUrl;
                    }
                  }
                } else if (url.startsWith("http")) {
                  const targetUrlObj = new URL(chunk.uri);
                  displayUrl = targetUrlObj.hostname;
                }
              }

              // Ensure actualLink has a protocol
              if (actualLink !== "#" && !actualLink.startsWith("http")) {
                actualLink = `https://${actualLink}`;
              }
            } catch (e) {
              console.error("Error parsing source URL:", e);
            }

            const handleLinkClick = async (e: React.MouseEvent) => {
              e.preventDefault();
              e.stopPropagation();
              if (actualLink && actualLink !== "#") {
                try {
                  await openUrl(actualLink);
                } catch (err) {
                  console.error("Failed to open URL:", err);
                }
              }
            };

            return (
              <a
                key={i}
                href={actualLink}
                onClick={handleLinkClick}
                className="flex flex-col gap-0.5 rounded p-1 text-left hover:bg-[var(--color-text)]/5 w-full cursor-pointer no-underline group/source"
              >
                <span className="max-w-[280px] truncate font-medium text-[var(--color-primary)] group-hover/source:underline">
                  {chunk.title || displayUrl || "Source"}
                </span>
                {displayUrl && !isVertexRedirect && (
                  <span className="max-w-[280px] truncate text-[var(--color-text)]/40 text-[10px]">
                    {displayUrl}
                  </span>
                )}
              </a>
            );
          })}
        </div>
      )}
    </div>
  );
};

const AssistantActionBar: FC = () => {
  const thread = useThread();
  const message = useMessage();

  const handleFork = async () => {
    // Find the index of the current message
    const messages = thread.messages;
    const messageIndex = messages.findIndex((m) => m.id === message.id);
    if (messageIndex === -1) return;

    // Get all messages up to and including current message, filtering for only user/assistant
    const forkedMessages = messages
      .slice(0, messageIndex + 1)
      .filter((msg) => msg.role === "user" || msg.role === "assistant")
      .map((msg) => ({
        role: msg.role,
        content: msg.content
          .filter(
            (part): part is { type: "text"; text: string } =>
              part.type === "text",
          )
          .map((part) => part.text)
          .join(""),
      }));

    try {
      await commands.openChatWindowWithMessages(forkedMessages);
    } catch (error) {
      console.error("Failed to fork conversation:", error);
    }
  };

  return (
    <ActionBarPrimitive.Root
      // hideWhenRunning // Disabled to prevent layout shift
      // autohide="not-last" // Disabled to prevent layout shift
      className="flex gap-1 text-[var(--color-text)]/50"
    >
      <ActionBarPrimitive.Copy asChild>
        <button className="rounded p-1 hover:bg-[var(--color-text)]/10">
          <CopyIcon className="h-4 w-4" />
        </button>
      </ActionBarPrimitive.Copy>
      <button
        onClick={handleFork}
        className="rounded p-1 hover:bg-[var(--color-text)]/10"
        title="Fork conversation from this message"
      >
        <GitForkIcon className="h-4 w-4" />
      </button>
      <ActionBarPrimitive.Reload asChild>
        <button className="rounded p-1 hover:bg-[var(--color-text)]/10">
          <RefreshCwIcon className="h-4 w-4" />
        </button>
      </ActionBarPrimitive.Reload>
    </ActionBarPrimitive.Root>
  );
};

const BranchPicker: FC = () => {
  return (
    <BranchPickerPrimitive.Root
      hideWhenSingleBranch
      className="inline-flex items-center text-xs text-[var(--color-text)]/50"
    >
      <BranchPickerPrimitive.Previous asChild>
        <button className="rounded p-1 hover:bg-[var(--color-text)]/10">
          <ChevronLeftIcon className="h-4 w-4" />
        </button>
      </BranchPickerPrimitive.Previous>
      <span className="font-medium">
        <BranchPickerPrimitive.Number /> / <BranchPickerPrimitive.Count />
      </span>
      <BranchPickerPrimitive.Next asChild>
        <button className="rounded p-1 hover:bg-[var(--color-text)]/10">
          <ChevronRightIcon className="h-4 w-4" />
        </button>
      </BranchPickerPrimitive.Next>
    </BranchPickerPrimitive.Root>
  );
};
