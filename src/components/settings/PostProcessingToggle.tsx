import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface PostProcessingToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PostProcessingToggle: React.FC<PostProcessingToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("coherent_enabled") || false;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(enabled) => updateSetting("coherent_enabled", enabled)}
        isUpdating={isUpdating("coherent_enabled")}
        label={t("settings.debug.postProcessingToggle.label")}
        description={t("settings.debug.postProcessingToggle.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
