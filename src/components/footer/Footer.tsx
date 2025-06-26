import React, { useState, useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

const Footer: React.FC = () => {
  const [isChecking, setIsChecking] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [isUpdating, setIsUpdating] = useState(false);
  const [version, setVersion] = useState("");
  const [showUpToDate, setShowUpToDate] = useState(false);
  const [isManualCheck, setIsManualCheck] = useState(false);

  useEffect(() => {
    // Get version from Tauri app info
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.3"); // fallback version
      }
    };

    fetchVersion();

    // Automatically check for updates on launch
    checkForUpdates();

    // Listen for menu-triggered update checks
    const unlisten = listen("check-for-updates", () => {
      checkForUpdates();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const checkForUpdates = async () => {
    // Don't check again if already checking
    if (isChecking) return;

    try {
      setIsChecking(true);
      const update = await check();

      if (update) {
        setUpdateAvailable(true);
        setShowUpToDate(false);
        console.log("Update available:", update.version);
      } else {
        console.log("No updates available");
        // Reset update available state in case it was previously true
        setUpdateAvailable(false);

        // Show "up to date" message only for manual checks
        if (isManualCheck) {
          setShowUpToDate(true);
          // Hide the message after 3 seconds
          setTimeout(() => setShowUpToDate(false), 3000);
        }
      }
    } catch (error) {
      console.error("Failed to check for updates:", error);
    } finally {
      setIsChecking(false);
      setIsManualCheck(false);
    }
  };

  const handleManualUpdateCheck = () => {
    setIsManualCheck(true);
    checkForUpdates();
  };

  const installUpdate = async () => {
    try {
      setIsUpdating(true);
      const update = await check();

      if (update) {
        console.log("Installing update...");
        await update.downloadAndInstall();

        // Restart the app to apply the update
        await relaunch();
      }
    } catch (error) {
      console.error("Failed to install update:", error);
      setIsUpdating(false);
    }
  };

  return (
    <div className="w-full border-t border-mid-gray/20 mt-4 pt-3">
      <div className="flex justify-between items-center text-xs px-4 pb-3 text-text/60">
        <div className="flex items-center">
          <span>v{version}</span>
        </div>

        <div className="flex items-center gap-2">
          {updateAvailable ? (
            <button
              onClick={installUpdate}
              disabled={isUpdating}
              className="text-logo-primary hover:text-logo-primary/80 transition-colors disabled:opacity-50 font-medium"
            >
              {isUpdating ? "Installing..." : "Update available"}
            </button>
          ) : showUpToDate ? (
            <span className="text-text/60">Up to date</span>
          ) : (
            <button
              onClick={handleManualUpdateCheck}
              disabled={isChecking}
              className="text-text/60 hover:text-text/80 transition-colors disabled:opacity-50"
            >
              {isChecking ? "Checking..." : "Check for updates"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default Footer;
