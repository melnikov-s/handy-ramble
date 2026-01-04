import { useState, useEffect, useCallback } from "react";
import { commands, ChatMessage, SavedChat, ChatSummary } from "@/bindings";
import { listen } from "@tauri-apps/api/event";

export function useChatPersistence() {
  const [savedChats, setSavedChats] = useState<ChatSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  const fetchChats = useCallback(async () => {
    try {
      setIsLoading(true);
      const result = await commands.listSavedChats();
      if (result.status === "ok") {
        setSavedChats(result.data);
      }
    } catch (err) {
      console.error("Failed to fetch saved chats:", err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchChats();

    // Listen for chat updates from backend
    const unlisten = listen("chats-updated", () => {
      fetchChats();
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, [fetchChats]);

  const saveChat = async (messages: ChatMessage[], title?: string) => {
    const result = await commands.saveChat(title || null, messages);
    if (result.status === "ok") {
      fetchChats();
      return result.data;
    }
    throw new Error(result.error);
  };

  const updateChat = async (id: number, messages: ChatMessage[]) => {
    const result = await commands.updateChat(id, messages);
    if (result.status === "ok") {
      // We don't necessarily need to fetch all chats again if only messages changed
      // but updated_at changes, so it's safer to fetch
      fetchChats();
      return;
    }
    throw new Error(result.error);
  };

  const updateChatTitle = async (id: number, title: string) => {
    const result = await commands.updateChatTitle(id, title);
    if (result.status === "ok") {
      fetchChats();
      return;
    }
    throw new Error(result.error);
  };

  const getChat = async (id: number): Promise<SavedChat | null> => {
    const result = await commands.getChat(id);
    if (result.status === "ok") {
      return result.data;
    }
    throw new Error(result.error);
  };

  const deleteChat = async (id: number) => {
    const result = await commands.deleteSavedChat(id);
    if (result.status === "ok") {
      fetchChats();
      return;
    }
    throw new Error(result.error);
  };

  const generateTitle = async (userMsg: string, assistantMsg: string) => {
    const result = await commands.generateChatTitle(userMsg, assistantMsg);
    if (result.status === "ok") {
      return result.data;
    }
    throw new Error(result.error);
  };

  const openSavedChat = async (id: number) => {
    const result = await commands.openSavedChat(id);
    if (result.status === "ok") {
      return result.data;
    }
    throw new Error(result.error);
  };

  return {
    savedChats,
    isLoading,
    saveChat,
    updateChat,
    updateChatTitle,
    getChat,
    deleteChat,
    generateTitle,
    openSavedChat,
    refreshChats: fetchChats,
  };
}
