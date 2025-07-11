import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToggleSwitch } from "../ui/ToggleSwitch";

interface PushToTalkProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PushToTalk: React.FC<PushToTalkProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const [pttEnabled, setPttEnabled] = useState<boolean>(false);
  const [isUpdating, setIsUpdating] = useState<boolean>(false);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      // Load from the store since this setting is handled differently
      const { load } = await import("@tauri-apps/plugin-store");
      const store = await load("settings_store.json", { autoSave: false });
      const settings = await store.get("settings");

      if (
        settings &&
        typeof settings === "object" &&
        "push_to_talk" in settings
      ) {
        setPttEnabled(settings.push_to_talk as boolean);
      }
    } catch (error) {
      console.error("Failed to load push-to-talk setting:", error);
    }
  };

  const handlePttToggle = async (enabled: boolean) => {
    if (isUpdating) return;

    try {
      setIsUpdating(true);
      await invoke("change_ptt_setting", { enabled });
      setPttEnabled(enabled);
      console.log(`Push-to-talk ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to update push-to-talk setting:", error);
      // Revert the toggle if the update failed
      setPttEnabled(!enabled);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <ToggleSwitch
      checked={pttEnabled}
      onChange={handlePttToggle}
      isUpdating={isUpdating}
      label="Push To Talk"
      description="Hold to record, release to stop"
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
};
