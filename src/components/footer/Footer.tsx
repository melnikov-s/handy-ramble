import React, { useState, useEffect, useRef } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

const Footer: React.FC = () => {
  const [isChecking, setIsChecking] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [version, setVersion] = useState("");
  const [showUpToDate, setShowUpToDate] = useState(false);

  const upToDateTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const isManualCheckRef = useRef(false);
  const downloadedBytesRef = useRef(0);
  const contentLengthRef = useRef(0);

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.2");
      }
    };

    fetchVersion();
    checkForUpdates();

    const unlisten = listen("check-for-updates", () => {
      handleManualUpdateCheck();
    });

    return () => {
      if (upToDateTimeoutRef.current) {
        clearTimeout(upToDateTimeoutRef.current);
      }
      unlisten.then((fn) => fn());
    };
  }, []);

  const checkForUpdates = async () => {
    if (isChecking) return;

    try {
      setIsChecking(true);
      const update = await check();

      if (update) {
        setUpdateAvailable(true);
        setShowUpToDate(false);
      } else {
        setUpdateAvailable(false);

        if (isManualCheckRef.current) {
          setShowUpToDate(true);
          if (upToDateTimeoutRef.current) {
            clearTimeout(upToDateTimeoutRef.current);
          }
          upToDateTimeoutRef.current = setTimeout(() => {
            setShowUpToDate(false);
          }, 3000);
        }
      }
    } catch (error) {
      console.error("Failed to check for updates:", error);
    } finally {
      setIsChecking(false);
      isManualCheckRef.current = false;
    }
  };

  const handleManualUpdateCheck = () => {
    isManualCheckRef.current = true;
    checkForUpdates();
  };

  const installUpdate = async () => {
    try {
      setIsInstalling(true);
      setDownloadProgress(0);
      downloadedBytesRef.current = 0;
      contentLengthRef.current = 0;
      const update = await check();

      if (!update) {
        console.log("No update available during install attempt");
        return;
      }

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            downloadedBytesRef.current = 0;
            contentLengthRef.current = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloadedBytesRef.current += event.data.chunkLength;
            const progress =
              contentLengthRef.current > 0
                ? Math.round(
                    (downloadedBytesRef.current / contentLengthRef.current) *
                      100,
                  )
                : 0;
            setDownloadProgress(Math.min(progress, 100));
            break;
        }
      });
      await relaunch();
    } catch (error) {
      console.error("Failed to install update:", error);
    } finally {
      setIsInstalling(false);
      setDownloadProgress(0);
      downloadedBytesRef.current = 0;
      contentLengthRef.current = 0;
    }
  };

  const getStatusText = () => {
    if (isInstalling) {
      return downloadProgress > 0 && downloadProgress < 100
        ? `Downloading... ${downloadProgress.toString().padStart(3)}%`
        : downloadProgress === 100
          ? "Installing..."
          : "Preparing...";
    }
    if (isChecking) return "Checking...";
    if (showUpToDate) return "Up to date";
    if (updateAvailable) return "Update available";
    return "Check for updates";
  };

  const getStatusAction = () => {
    if (updateAvailable && !isInstalling) return installUpdate;
    if (!isChecking && !isInstalling && !updateAvailable)
      return handleManualUpdateCheck;
    return undefined;
  };

  const isDisabled = isChecking || isInstalling;
  const isClickable =
    !isDisabled && (updateAvailable || (!isChecking && !showUpToDate));

  return (
    <div className="w-full border-t border-mid-gray/20 mt-4 pt-3">
      <div className="flex justify-between items-center text-xs px-4 pb-3 text-text/60">
        <div className="flex items-center">
          <span>v{version}</span>
        </div>

        <div className="flex items-center gap-3">
          {isClickable ? (
            <button
              onClick={getStatusAction()}
              disabled={isDisabled}
              className={`transition-colors disabled:opacity-50 tabular-nums ${
                updateAvailable
                  ? "text-logo-primary hover:text-logo-primary/80 font-medium"
                  : "text-text/60 hover:text-text/80"
              }`}
            >
              {getStatusText()}
            </button>
          ) : (
            <span className="text-text/60 tabular-nums">{getStatusText()}</span>
          )}

          {isInstalling && downloadProgress > 0 && downloadProgress < 100 && (
            <progress
              value={downloadProgress}
              max={100}
              className="w-20 h-2 [&::-webkit-progress-bar]:rounded-full [&::-webkit-progress-bar]:bg-mid-gray/20 [&::-webkit-progress-value]:rounded-full [&::-webkit-progress-value]:bg-logo-primary"
            />
          )}
        </div>
      </div>
    </div>
  );
};

export default Footer;
