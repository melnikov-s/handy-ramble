import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useChatPersistence } from "../../hooks/useChatPersistence";
import {
  MessageSquare,
  Trash2,
  ExternalLink,
  Search,
  Calendar,
  MessageCircle,
} from "lucide-react";
import { cn } from "@/lib/utils/cn";

export const ChatHistorySettings: React.FC = () => {
  const { t } = useTranslation();
  const { savedChats, isLoading, deleteChat, openSavedChat } =
    useChatPersistence();
  const [searchQuery, setSearchQuery] = useState("");

  const filteredChats = savedChats.filter((chat) =>
    chat.title.toLowerCase().includes(searchQuery.toLowerCase()),
  );

  const formatDate = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  return (
    <div className="flex flex-col w-full max-w-3xl space-y-6">
      <div className="flex flex-col space-y-2">
        <h2 className="text-2xl font-bold text-[var(--color-text)]">
          {t("sidebar.chats")}
        </h2>
        <p className="text-sm text-[var(--color-text)]/60">
          Access and manage your persistent chat history.
        </p>
      </div>

      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-[var(--color-text)]/40" />
        <input
          type="text"
          placeholder="Search chats..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full bg-[var(--color-text)]/5 border border-[var(--color-text)]/10 rounded-lg py-2 pl-10 pr-4 text-sm focus:outline-none focus:ring-1 focus:ring-logo-primary transition-all"
        />
      </div>

      <div className="space-y-3">
        {isLoading ? (
          <div className="flex justify-center py-10 text-[var(--color-text)]/40">
            <span className="text-sm">Loading chats...</span>
          </div>
        ) : filteredChats.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-20 bg-[var(--color-text)]/5 rounded-xl border border-dashed border-[var(--color-text)]/10">
            <MessageSquare className="h-10 w-10 text-[var(--color-text)]/20 mb-3" />
            <p className="text-sm text-[var(--color-text)]/40">
              {searchQuery
                ? "No chats match your search."
                : "No saved chats found."}
            </p>
          </div>
        ) : (
          filteredChats.map((chat) => (
            <div
              key={chat.id}
              className="group flex items-center justify-between p-4 bg-[var(--color-text)]/5 hover:bg-[var(--color-text)]/10 border border-[var(--color-text)]/10 rounded-xl transition-all cursor-pointer"
              onClick={() => openSavedChat(chat.id)}
            >
              <div className="flex items-start gap-4 flex-1 min-w-0">
                <div className="mt-1 p-2 bg-logo-primary/10 rounded-lg">
                  <MessageCircle className="h-5 w-5 text-logo-primary" />
                </div>
                <div className="flex-1 min-w-0">
                  <h3 className="text-sm font-semibold text-[var(--color-text)] truncate group-hover:text-logo-primary transition-colors">
                    {chat.title}
                  </h3>
                  <div className="flex items-center gap-4 mt-1">
                    <div className="flex items-center gap-1.5 text-xs text-[var(--color-text)]/40">
                      <Calendar className="h-3 w-3" />
                      {formatDate(chat.updated_at)}
                    </div>
                    <div className="flex items-center gap-1.5 text-xs text-[var(--color-text)]/40">
                      <MessageSquare className="h-3 w-3" />
                      {chat.message_count} messages
                    </div>
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    openSavedChat(chat.id);
                  }}
                  className="p-2 hover:bg-[var(--color-text)]/10 rounded-lg text-[var(--color-text)]/60 hover:text-logo-primary transition-colors"
                  title="Open Chat"
                >
                  <ExternalLink className="h-4 w-4" />
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    if (confirm("Are you sure you want to delete this chat?")) {
                      deleteChat(chat.id);
                    }
                  }}
                  className="p-2 hover:bg-red-500/10 rounded-lg text-[var(--color-text)]/60 hover:text-red-500 transition-colors"
                  title="Delete Chat"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};

export default ChatHistorySettings;
