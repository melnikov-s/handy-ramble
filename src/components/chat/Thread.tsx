import {
  ActionBarPrimitive,
  BranchPickerPrimitive,
  ComposerPrimitive,
  MessagePrimitive,
  ThreadPrimitive,
} from "@assistant-ui/react";
import {
  ArrowUpIcon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  CopyIcon,
  RefreshCwIcon,
  XIcon,
} from "lucide-react";
import type { FC } from "react";
import { cn } from "@/lib/utils/cn";
import { Button } from "@/components/ui/Button";

export const Thread: FC = () => {
  return (
    <ThreadPrimitive.Root className="aui-root flex h-full flex-col bg-[var(--color-background)]">
      <ThreadPrimitive.Viewport className="flex flex-1 flex-col overflow-y-auto scroll-smooth px-4 pt-4">
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

      <div className="sticky bottom-0 mx-auto w-full max-w-2xl px-4 pb-4">
        <Composer />
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

const Composer: FC = () => {
  return (
    <ComposerPrimitive.Root className="flex w-full items-end gap-2 rounded-lg border border-[var(--color-text)]/20 bg-[var(--color-background)] p-2">
      <ComposerPrimitive.Input
        placeholder="Type a message..."
        className="min-h-[40px] flex-1 resize-none bg-transparent px-2 py-2 text-sm text-[var(--color-text)] placeholder:text-[var(--color-text)]/50 focus:outline-none"
        autoFocus
      />
      <ComposerPrimitive.Send asChild>
        <Button
          variant="primary"
          size="sm"
          className="h-8 w-8 rounded-full p-0"
        >
          <ArrowUpIcon className="h-4 w-4" />
        </Button>
      </ComposerPrimitive.Send>
    </ComposerPrimitive.Root>
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
  return (
    <MessagePrimitive.Root className="mb-4 flex flex-col group">
      <div className="max-w-[80%] rounded-lg bg-[var(--color-text)]/10 px-4 py-2 text-[var(--color-text)]">
        <MessagePrimitive.Content />
      </div>
      <AssistantActionBar />
    </MessagePrimitive.Root>
  );
};

const AssistantActionBar: FC = () => {
  return (
    <ActionBarPrimitive.Root
      hideWhenRunning
      autohide="not-last"
      className="mt-1 flex gap-1 text-[var(--color-text)]/50 invisible opacity-0 group-hover:visible group-hover:opacity-100 transition-all duration-200"
    >
      <ActionBarPrimitive.Copy asChild>
        <button className="rounded p-1 hover:bg-[var(--color-text)]/10">
          <CopyIcon className="h-4 w-4" />
        </button>
      </ActionBarPrimitive.Copy>
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
