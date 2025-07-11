import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ToggleSwitch } from "../ui/ToggleSwitch";

interface AlwaysOnMicrophoneProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AlwaysOnMicrophone: React.FC<AlwaysOnMicrophoneProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const [alwaysOnMode, setAlwaysOnMode] = useState<boolean>(false);
  const [isUpdating, setIsUpdating] = useState<boolean>(false);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const alwaysOn: boolean = await invoke("get_microphone_mode");
      setAlwaysOnMode(alwaysOn);
    } catch (error) {
      console.error("Failed to load always-on microphone setting:", error);
    }
  };

  const handleAlwaysOnToggle = async (enabled: boolean) => {
    if (isUpdating) return;

    try {
      setIsUpdating(true);
      await invoke("update_microphone_mode", { alwaysOn: enabled });
      setAlwaysOnMode(enabled);

      // Provide user feedback about the change
      console.log(`Always-on microphone ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to update microphone mode:", error);
      // Revert the toggle if the update failed
      setAlwaysOnMode(!enabled);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <ToggleSwitch
      checked={alwaysOnMode}
      onChange={handleAlwaysOnToggle}
      isUpdating={isUpdating}
      label="Always-On Microphone"
      description="Keep microphone active for low latency recording. This may prevent your computer from sleeping."
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
};
