import React from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { stat } from "@tauri-apps/plugin-fs";
import { commands } from "@/bindings";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { useSettings } from "../../hooks/useSettings";

interface SystemPromptFileProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const SystemPromptFile: React.FC<SystemPromptFileProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, refreshSettings } = useSettings();

    const systemPromptFile = getSetting("system_prompt_file") as
      | string
      | null
      | undefined;
    const fileName = systemPromptFile
      ? systemPromptFile.split("/").pop() || systemPromptFile
      : null;

    const handleSelectFile = async () => {
      try {
        // Don't use extension filters - they cause issues with directories ending in .md
        const selected = await open({
          multiple: false,
          directory: false,
        });

        if (selected && typeof selected === "string") {
          // Verify it's a file (not a directory) and has a valid extension
          const fileInfo = await stat(selected);
          if (fileInfo.isDirectory) {
            console.error("Selected path is a directory, not a file");
            return;
          }

          const validExtensions = [".txt", ".md", ".markdown"];
          const hasValidExtension = validExtensions.some((ext) =>
            selected.toLowerCase().endsWith(ext),
          );
          if (!hasValidExtension) {
            console.error("Selected file does not have a valid extension");
            return;
          }

          await commands.changeSystemPromptFileSetting(selected);
          await refreshSettings();
        }
      } catch (error) {
        console.error("Failed to select system prompt file:", error);
      }
    };

    const handleClear = async () => {
      try {
        await commands.changeSystemPromptFileSetting(null);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to clear system prompt file:", error);
      }
    };

    return (
      <SettingContainer
        title={t("settings.general.systemPromptFile.title")}
        description={t("settings.general.systemPromptFile.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <div className="flex items-center gap-2">
          {fileName ? (
            <>
              <div className="flex-1 min-w-0 px-2 py-2 bg-mid-gray/10 border border-mid-gray/80 rounded text-xs font-mono truncate">
                {fileName}
              </div>
              <Button
                onClick={handleSelectFile}
                variant="secondary"
                size="sm"
                className="px-3 py-2"
              >
                {t("common.change")}
              </Button>
              <Button
                onClick={handleClear}
                variant="secondary"
                size="sm"
                className="px-3 py-2"
              >
                {t("common.clear")}
              </Button>
            </>
          ) : (
            <>
              <span className="flex-1 text-sm text-mid-gray/70">
                {t("settings.general.systemPromptFile.notSet")}
              </span>
              <Button
                onClick={handleSelectFile}
                variant="primary"
                size="sm"
                className="px-3 py-2"
              >
                {t("settings.general.systemPromptFile.selectFile")}
              </Button>
            </>
          )}
        </div>
      </SettingContainer>
    );
  },
);

SystemPromptFile.displayName = "SystemPromptFile";
