import React, { useState, useRef, useEffect } from "react";
import { AudioDevice } from "../../lib/types";

interface DropdownProps {
  devices: AudioDevice[];
  selectedDevice: string | null;
  onSelect: (deviceName: string) => void;
  placeholder?: string;
  disabled?: boolean;
}

export const Dropdown: React.FC<DropdownProps> = ({
  devices,
  selectedDevice,
  onSelect,
  placeholder = "Select a microphone...",
  disabled = false,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  // Find the selected device name
  const selectedDeviceName = selectedDevice
    ? devices.find((d) => d.name === selectedDevice)?.name || "Unknown Device"
    : null;

  const handleSelect = (deviceName: string) => {
    onSelect(deviceName);
    setIsOpen(false);
  };

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        type="button"
        className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded min-w-[200px] text-left flex items-center justify-between transition-all duration-150 ${
          disabled
            ? "opacity-50 cursor-not-allowed"
            : "hover:bg-logo-primary/10 cursor-pointer hover:border-logo-primary"
        }`}
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
      >
        <span className="truncate">{selectedDeviceName || placeholder}</span>
        <svg
          className={`w-4 h-4 ml-2 transition-transform duration-200 ${
            isOpen ? "transform rotate-180" : ""
          }`}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M19 9l-7 7-7-7"
          />
        </svg>
      </button>

      {isOpen && !disabled && (
        <div className="absolute top-full left-0 right-0 mt-1 bg-background border border-mid-gray/80 rounded shadow-lg z-50 max-h-60 overflow-y-auto">
          {devices.length === 0 ? (
            <div className="px-2 py-1 text-sm text-mid-gray">
              No microphones found
            </div>
          ) : (
            devices.map((device) => (
              <button
                key={device.index}
                type="button"
                className={`w-full px-2 py-1 text-sm text-left hover:bg-logo-primary/10 transition-colors duration-150 ${
                  selectedDevice === device.name
                    ? "bg-logo-primary/20 text-logo-primary font-semibold"
                    : ""
                }`}
                onClick={() => handleSelect(device.name)}
              >
                <div className="flex items-center justify-between">
                  <span className="truncate">{device.name}</span>
                </div>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
};
