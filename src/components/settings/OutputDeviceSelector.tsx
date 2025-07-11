import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AudioDevice } from "../../lib/types";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import ResetIcon from "../icons/ResetIcon";

interface OutputDeviceSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const OutputDeviceSelector: React.FC<OutputDeviceSelectorProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const [outputDevices, setOutputDevices] = useState<AudioDevice[]>([]);
  const [selectedOutputDevice, setSelectedOutputDevice] =
    useState<string>("default");
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [isUpdating, setIsUpdating] = useState<boolean>(false);

  useEffect(() => {
    loadOutputDevices();
    loadSettings();
  }, []);

  const loadOutputDevices = async () => {
    try {
      const devices: AudioDevice[] = await invoke(
        "get_available_output_devices",
      );
      setOutputDevices(devices);
    } catch (error) {
      console.error("Failed to load output devices:", error);
      setOutputDevices([]);
    }
  };

  const loadSettings = async () => {
    try {
      setIsLoading(true);
      const selectedDevice: string = await invoke("get_selected_output_device");
      setSelectedOutputDevice(selectedDevice);
    } catch (error) {
      console.error("Failed to load output device settings:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleOutputDeviceSelect = async (deviceName: string) => {
    if (isUpdating) return;

    try {
      setIsUpdating(true);
      await invoke("set_selected_output_device", { deviceName });
      setSelectedOutputDevice(deviceName);
      console.log(
        `Output device changed to: ${outputDevices.find((d) => d.name === deviceName)?.name}`,
      );
    } catch (error) {
      console.error("Failed to set output device:", error);
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
      await invoke("set_selected_output_device", { deviceName: "default" });
      setSelectedOutputDevice("default");
      console.log("Output device reset to default");
    } catch (error) {
      console.error("Failed to reset output device:", error);
      // Revert selection if reset failed
      loadSettings();
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <SettingContainer
      title="Output Device"
      description="Select your preferred audio output device for feedback sounds"
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <div className="flex items-center space-x-1">
        <Dropdown
          devices={outputDevices}
          selectedDevice={selectedOutputDevice}
          onSelect={handleOutputDeviceSelect}
          placeholder={isLoading ? "Loading..." : "Select output device..."}
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
