import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { AudioPlayer } from "../../ui/AudioPlayer";
import { Button } from "../../ui/Button";
import { Copy, Star, Check, Trash2, FolderOpen } from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { commands, type HistoryEntry } from "@/bindings";
import { formatDateTime } from "@/utils/dateFormat";

interface OpenRecordingsButtonProps {
  onClick: () => void;
  label: string;
}

const OpenRecordingsButton: React.FC<OpenRecordingsButtonProps> = ({
  onClick,
  label,
}) => (
  <Button
    onClick={onClick}
    variant="secondary"
    size="sm"
    className="flex items-center gap-2"
    title={label}
  >
    <FolderOpen className="w-4 h-4" />
    <span>{label}</span>
  </Button>
);

export const HistorySettings: React.FC = () => {
  const { t } = useTranslation();
  const [historyEntries, setHistoryEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const loadHistoryEntries = useCallback(async () => {
    try {
      const result = await commands.getHistoryEntries();
      if (result.status === "ok") {
        setHistoryEntries(result.data);
      }
    } catch (error) {
      console.error("Failed to load history entries:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadHistoryEntries();

    // Listen for history update events
    const setupListener = async () => {
      const unlisten = await listen("history-updated", () => {
        console.log("History updated, reloading entries...");
        loadHistoryEntries();
      });

      // Return cleanup function
      return unlisten;
    };

    let unlistenPromise = setupListener();

    return () => {
      unlistenPromise.then((unlisten) => {
        if (unlisten) {
          unlisten();
        }
      });
    };
  }, [loadHistoryEntries]);

  const toggleSaved = async (id: number) => {
    try {
      await commands.toggleHistoryEntrySaved(id);
      // No need to reload here - the event listener will handle it
    } catch (error) {
      console.error("Failed to toggle saved status:", error);
    }
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
    }
  };

  const getAudioUrl = async (fileName: string) => {
    try {
      const result = await commands.getAudioFilePath(fileName);
      if (result.status === "ok") {
        return convertFileSrc(`${result.data}`, "asset");
      }
      return null;
    } catch (error) {
      console.error("Failed to get audio file path:", error);
      return null;
    }
  };

  const deleteAudioEntry = async (id: number) => {
    try {
      await commands.deleteHistoryEntry(id);
    } catch (error) {
      console.error("Failed to delete audio entry:", error);
      throw error;
    }
  };

  const openRecordingsFolder = async () => {
    try {
      await commands.openRecordingsFolder();
    } catch (error) {
      console.error("Failed to open recordings folder:", error);
    }
  };

  if (loading) {
    return (
      <div className="max-w-3xl w-full mx-auto space-y-6">
        <div className="space-y-2">
          <div className="px-4 flex items-center justify-between">
            <div>
              <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
                {t("settings.history.title")}
              </h2>
            </div>
            <OpenRecordingsButton
              onClick={openRecordingsFolder}
              label={t("settings.history.openFolder")}
            />
          </div>
          <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
            <div className="px-4 py-3 text-center text-text/60">
              {t("settings.history.loading")}
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (historyEntries.length === 0) {
    return (
      <div className="max-w-3xl w-full mx-auto space-y-6">
        <div className="space-y-2">
          <div className="px-4 flex items-center justify-between">
            <div>
              <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
                {t("settings.history.title")}
              </h2>
            </div>
            <OpenRecordingsButton
              onClick={openRecordingsFolder}
              label={t("settings.history.openFolder")}
            />
          </div>
          <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
            <div className="px-4 py-3 text-center text-text/60">
              {t("settings.history.empty")}
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="space-y-2">
        <div className="px-4 flex items-center justify-between">
          <div>
            <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
              {t("settings.history.title")}
            </h2>
          </div>
          <OpenRecordingsButton
            onClick={openRecordingsFolder}
            label={t("settings.history.openFolder")}
          />
        </div>
        <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
          <div className="divide-y divide-mid-gray/20">
            {historyEntries.map((entry) => (
              <HistoryEntryComponent
                key={entry.id}
                entry={entry}
                onToggleSaved={() => toggleSaved(entry.id)}
                getAudioUrl={getAudioUrl}
                deleteAudio={deleteAudioEntry}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

interface HistoryEntryProps {
  entry: HistoryEntry;
  onToggleSaved: () => void;
  getAudioUrl: (fileName: string) => Promise<string | null>;
  deleteAudio: (id: number) => Promise<void>;
}

const HistoryEntryComponent: React.FC<HistoryEntryProps> = ({
  entry,
  onToggleSaved,
  getAudioUrl,
  deleteAudio,
}) => {
  const { t, i18n } = useTranslation();
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [showCopiedOriginal, setShowCopiedOriginal] = useState(false);
  const [showCopiedRefined, setShowCopiedRefined] = useState(false);

  useEffect(() => {
    const loadAudio = async () => {
      const url = await getAudioUrl(entry.file_name);
      setAudioUrl(url);
    };
    loadAudio();
  }, [entry.file_name, getAudioUrl]);

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch (error) {
      console.error("Failed to copy to clipboard:", error);
      return false;
    }
  };

  const handleCopyOriginal = async () => {
    const success = await copyToClipboard(entry.transcription_text);
    if (success) {
      setShowCopiedOriginal(true);
      setTimeout(() => setShowCopiedOriginal(false), 2000);
    }
  };

  const handleCopyRefined = async () => {
    if (entry.post_processed_text) {
      const success = await copyToClipboard(entry.post_processed_text);
      if (success) {
        setShowCopiedRefined(true);
        setTimeout(() => setShowCopiedRefined(false), 2000);
      }
    }
  };

  const handleDeleteEntry = async () => {
    try {
      await deleteAudio(entry.id);
    } catch (error) {
      console.error("Failed to delete entry:", error);
      alert(t("settings.history.deleteError"));
    }
  };

  const formattedDate = formatDateTime(String(entry.timestamp), i18n.language);
  const hasRefinedText = !!entry.post_processed_text;
  const isFailed = entry.transcription_status === "failed";
  const isPending = entry.transcription_status === "pending";

  return (
    <div className="px-4 py-2 pb-5 flex flex-col gap-3">
      {/* Header with date and action buttons */}
      <div className="flex justify-between items-center">
        <div className="flex items-center gap-2">
          <p className="text-sm font-medium">{formattedDate}</p>
          {isFailed && (
            <span className="text-xs bg-red-500/20 text-red-400 px-2 py-0.5 rounded">
              {t("settings.history.failed")}
            </span>
          )}
          {isPending && (
            <span className="text-xs bg-yellow-500/20 text-yellow-400 px-2 py-0.5 rounded">
              {t("settings.history.processing")}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={onToggleSaved}
            className={`p-2 rounded transition-colors cursor-pointer ${
              entry.saved
                ? "text-logo-primary hover:text-logo-primary/80"
                : "text-text/50 hover:text-logo-primary"
            }`}
            title={
              entry.saved
                ? t("settings.history.unsave")
                : t("settings.history.save")
            }
          >
            <Star
              width={16}
              height={16}
              fill={entry.saved ? "currentColor" : "none"}
            />
          </button>
          <button
            onClick={handleDeleteEntry}
            className="text-text/50 hover:text-logo-primary transition-colors cursor-pointer"
            title={t("settings.history.delete")}
          >
            <Trash2 width={16} height={16} />
          </button>
        </div>
      </div>

      {/* Error message for failed transcriptions */}
      {isFailed && entry.transcription_error && (
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3">
          <p className="text-sm text-red-400">
            {t("settings.history.transcriptionFailed")}
          </p>
          <p className="text-xs text-red-400/70 mt-1 font-mono">
            {entry.transcription_error}
          </p>
        </div>
      )}

      {/* Text content - only show if not failed/pending */}
      {!isFailed && !isPending && (
        <>
          {hasRefinedText ? (
            // Two-section layout: Original + Refined
            <div className="flex flex-col gap-3">
              {/* Refined text - primary/prominent */}
              <div className="border-l-2 border-logo-primary/60 pl-3">
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs font-medium text-logo-primary/80 uppercase tracking-wide">
                    {t("settings.history.refined")}
                  </span>
                  <button
                    onClick={handleCopyRefined}
                    className="text-text/50 hover:text-logo-primary transition-colors cursor-pointer p-1"
                    title={t("settings.history.copyRefined")}
                  >
                    {showCopiedRefined ? (
                      <Check width={14} height={14} />
                    ) : (
                      <Copy width={14} height={14} />
                    )}
                  </button>
                </div>
                <p className="text-text/90 text-sm">
                  {entry.post_processed_text}
                </p>
              </div>

              {/* Original text - secondary/muted */}
              <div className="border-l-2 border-mid-gray/40 pl-3">
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs font-medium text-mid-gray uppercase tracking-wide">
                    {t("settings.history.original")}
                  </span>
                  <button
                    onClick={handleCopyOriginal}
                    className="text-text/50 hover:text-logo-primary transition-colors cursor-pointer p-1"
                    title={t("settings.history.copyOriginal")}
                  >
                    {showCopiedOriginal ? (
                      <Check width={14} height={14} />
                    ) : (
                      <Copy width={14} height={14} />
                    )}
                  </button>
                </div>
                <p className="italic text-text/60 text-sm">
                  {entry.transcription_text}
                </p>
              </div>
            </div>
          ) : (
            // Single-section layout: Original only (with copy button)
            <div className="flex items-start justify-between gap-2">
              <p className="italic text-text/90 text-sm flex-1">
                {entry.transcription_text}
              </p>
              <button
                onClick={handleCopyOriginal}
                className="text-text/50 hover:text-logo-primary transition-colors cursor-pointer p-1 flex-shrink-0"
                title={t("settings.history.copyToClipboard")}
              >
                {showCopiedOriginal ? (
                  <Check width={14} height={14} />
                ) : (
                  <Copy width={14} height={14} />
                )}
              </button>
            </div>
          )}
        </>
      )}

      {audioUrl && <AudioPlayer src={audioUrl} className="w-full" />}
    </div>
  );
};
