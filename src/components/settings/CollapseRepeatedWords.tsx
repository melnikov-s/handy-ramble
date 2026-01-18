import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { commands } from "@/bindings";

interface CollapseRepeatedWordsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const CollapseRepeatedWords: React.FC<CollapseRepeatedWordsProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { settings, refreshSettings } = useSettings();
    const [isUpdating, setIsUpdating] = useState(false);

    const isEnabled = settings?.collapse_repeated_words ?? true;

    const handleToggle = async (enabled: boolean) => {
      setIsUpdating(true);
      try {
        const result =
          await commands.changeCollapseRepeatedWordsSetting(enabled);
        if (result.status === "ok") {
          await refreshSettings();
        }
      } finally {
        setIsUpdating(false);
      }
    };

    return (
      <ToggleSwitch
        checked={isEnabled}
        onChange={handleToggle}
        disabled={isUpdating}
        label={t(
          "settings.advanced.collapseRepeatedWords.title",
          "Collapse Repeated Words",
        )}
        description={t(
          "settings.advanced.collapseRepeatedWords.description",
          "Automatically collapse repeated words caused by model hallucinations (e.g., 'I I I am' → 'I am').",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
