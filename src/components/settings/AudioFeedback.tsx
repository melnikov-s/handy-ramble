import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToggleSwitch } from "../ui/ToggleSwitch";

interface AudioFeedbackProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AudioFeedback: React.FC<AudioFeedbackProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const [audioFeedbackEnabled, setAudioFeedbackEnabled] =
    useState<boolean>(false);
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
        "audio_feedback" in settings
      ) {
        setAudioFeedbackEnabled(settings.audio_feedback as boolean);
      }
    } catch (error) {
      console.error("Failed to load audio feedback setting:", error);
    }
  };

  const handleAudioFeedbackToggle = async (enabled: boolean) => {
    if (isUpdating) return;

    try {
      setIsUpdating(true);
      await invoke("change_audio_feedback_setting", { enabled });
      setAudioFeedbackEnabled(enabled);
      console.log(`Audio feedback ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to update audio feedback setting:", error);
      // Revert the toggle if the update failed
      setAudioFeedbackEnabled(!enabled);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <ToggleSwitch
      checked={audioFeedbackEnabled}
      onChange={handleAudioFeedbackToggle}
      isUpdating={isUpdating}
      label="Audio Feedback"
      description="Play sound when recording starts and stops"
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
};
