import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
} from "../components/icons";
import { Sparkles, AlertCircle, X } from "lucide-react";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import { syncLanguageFromSettings } from "@/i18n";

type OverlayState =
  | "recording"
  | "ramble_recording"
  | "transcribing"
  | "ramble_transcribing"
  | "making_coherent"
  | "error";

interface ErrorPayload {
  state: string;
  message: string;
}

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [errorMessage, setErrorMessage] = useState<string>("");
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));

  useEffect(() => {
    const setupEventListeners = async () => {
      // Listen for show-overlay event from Rust
      const unlistenShow = await listen("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();
        const overlayState = event.payload as OverlayState;
        setState(overlayState);
        setErrorMessage("");
        setIsVisible(true);
      });

      // Listen for error overlay event from Rust
      const unlistenError = await listen<ErrorPayload>(
        "show-overlay-error",
        async (event) => {
          await syncLanguageFromSettings();
          setState("error");
          setErrorMessage(event.payload.message);
          setIsVisible(true);
        },
      );

      // Listen for hide-overlay event from Rust
      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
        setErrorMessage("");
      });

      // Listen for mic-level updates
      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];

        // Apply smoothing to reduce jitter
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3; // Smooth transition
        });

        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 9));
      });

      // Cleanup function
      return () => {
        unlistenShow();
        unlistenError();
        unlistenHide();
        unlistenLevel();
      };
    };

    setupEventListeners();
  }, []);

  const handleDismissError = () => {
    setIsVisible(false);
    setErrorMessage("");
    setState("recording");
  };

  const getIcon = () => {
    if (state === "recording") {
      return <MicrophoneIcon color="#FAA2CA" />;
    } else if (state === "ramble_recording" || state === "making_coherent") {
      return <Sparkles size={16} style={{ color: "#00e5cc" }} />;
    } else if (state === "ramble_transcribing") {
      return <TranscriptionIcon color="#00e5cc" />;
    } else if (state === "error") {
      return <AlertCircle size={16} style={{ color: "#ff6b6b" }} />;
    } else {
      // transcribing state
      return <TranscriptionIcon color="#FAA2CA" />;
    }
  };

  return (
    <div
      className={`recording-overlay ${isVisible ? "fade-in" : ""} ${state === "ramble_recording" || state === "ramble_transcribing" || state === "making_coherent" ? "refining-mode" : ""} ${state === "error" ? "error-state" : ""}`}
    >
      <div className="overlay-left">{getIcon()}</div>

      <div className="overlay-middle">
        {state === "recording" && (
          <div className="stacked-content">
            <div className="mode-label dictating-label">
              {t("overlay.dictating", "Dictating")}
            </div>
            <div className="bars-container">
              {levels.map((v, i) => (
                <div
                  key={i}
                  className="bar"
                  style={{
                    height: `${Math.min(14, 3 + Math.pow(v, 0.7) * 11)}px`,
                    transition: "height 60ms ease-out, opacity 120ms ease-out",
                    opacity: Math.max(0.3, v * 1.5),
                  }}
                />
              ))}
            </div>
          </div>
        )}
        {state === "ramble_recording" && (
          <div className="stacked-content">
            <div className="mode-label refining-label">
              {t("overlay.refining", "Refining")}
            </div>
            <div className="bars-container refining-bars">
              {levels.map((v, i) => (
                <div
                  key={i}
                  className="bar refining-bar"
                  style={{
                    height: `${Math.min(14, 3 + Math.pow(v, 0.7) * 11)}px`,
                    transition: "height 60ms ease-out, opacity 120ms ease-out",
                    opacity: Math.max(0.3, v * 1.5),
                  }}
                />
              ))}
            </div>
          </div>
        )}
        {state === "transcribing" && (
          <div className="transcribing-text">{t("overlay.transcribing")}</div>
        )}
        {state === "ramble_transcribing" && (
          <div className="transcribing-text refining-transcribing">
            {t("overlay.transcribing")}
          </div>
        )}
        {state === "making_coherent" && (
          <div className="stacked-content">
            <div className="mode-label refining-label">
              {t("overlay.refining", "Refining")}
            </div>
            <div className="refining-indicator">
              <div className="refining-dot"></div>
              <div className="refining-dot"></div>
              <div className="refining-dot"></div>
            </div>
          </div>
        )}
        {state === "error" && (
          <div
            className="error-text text-red-400 text-xs truncate max-w-[120px]"
            title={errorMessage}
          >
            {errorMessage}
          </div>
        )}
      </div>

      <div className="overlay-right">
        {(state === "recording" || state === "ramble_recording") && (
          <div
            className={`cancel-button ${state === "ramble_recording" ? "refining-cancel" : ""}`}
            onClick={() => {
              commands.cancelOperation();
            }}
          >
            <CancelIcon
              color={state === "ramble_recording" ? "#00e5cc" : "#FAA2CA"}
            />
          </div>
        )}
        {state === "error" && (
          <div
            className="cancel-button"
            onClick={handleDismissError}
            title={t("overlay.dismissError", "Dismiss")}
          >
            <X className="w-4 h-4" />
          </div>
        )}
      </div>
    </div>
  );
};

export default RecordingOverlay;
