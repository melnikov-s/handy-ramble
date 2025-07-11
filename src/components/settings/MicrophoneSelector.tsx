import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AudioDevice } from "../../lib/types";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import ResetIcon from "../icons/ResetIcon";

interface MicrophoneSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const MicrophoneSelector: React.FC<MicrophoneSelectorProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const [audioDevices, setAudioDevices] = useState<AudioDevice[]>([]);
  const [selectedMicrophone, setSelectedMicrophone] =
    useState<string>("default");
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [isUpdating, setIsUpdating] = useState<boolean>(false);

  useEffect(() => {
    loadAudioDevices();
    loadSettings();
  }, []);

  const loadAudioDevices = async () => {
    try {
      const devices: AudioDevice[] = await invoke("get_available_microphones");
      setAudioDevices(devices);
    } catch (error) {
      console.error("Failed to load audio devices:", error);
      setAudioDevices([]);
    }
  };

  const loadSettings = async () => {
    try {
      setIsLoading(true);
      const selectedMic: string = await invoke("get_selected_microphone");
      setSelectedMicrophone(selectedMic);
    } catch (error) {
      console.error("Failed to load microphone settings:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleMicrophoneSelect = async (deviceName: string) => {
    if (isUpdating) return;

    try {
      setIsUpdating(true);
      await invoke("set_selected_microphone", { deviceName });
      setSelectedMicrophone(deviceName);
      console.log(
        `Microphone changed to: ${audioDevices.find((d) => d.name === deviceName)?.name}`,
      );
    } catch (error) {
      console.error("Failed to set microphone:", error);
      // Revert selection if update failed
      loadSettings();
    } finally {
      setIsUpdating(false);
    }
  };

  const handleReset = async () => {
    if (isUpdating) return;

    try {
      setIsUpdating(true);
      await invoke("set_selected_microphone", { deviceName: "default" });
      setSelectedMicrophone("default");
      console.log("Microphone reset to default");
    } catch (error) {
      console.error("Failed to reset microphone:", error);
      // Revert selection if reset failed
      loadSettings();
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <SettingContainer
      title="Microphone"
      description="Select your preferred microphone device"
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <div className="flex items-center space-x-1">
        <Dropdown
          devices={audioDevices}
          selectedDevice={selectedMicrophone}
          onSelect={handleMicrophoneSelect}
          placeholder={isLoading ? "Loading..." : "Select microphone..."}
          disabled={isUpdating || isLoading}
        />
        <button
          className="px-2 py-1 hover:bg-logo-primary/30 active:bg-logo-primary/50 active:scale-95 rounded fill-text hover:cursor-pointer hover:border-logo-primary border border-transparent transition-all duration-150"
          onClick={handleReset}
          disabled={isUpdating || isLoading}
        >
          <ResetIcon className="" />
        </button>
      </div>
      {isUpdating && (
        <div className="absolute inset-0 bg-mid-gray/10 rounded flex items-center justify-center">
          <div className="w-4 h-4 border-2 border-logo-primary border-t-transparent rounded-full animate-spin"></div>
        </div>
      )}
    </SettingContainer>
  );
};
