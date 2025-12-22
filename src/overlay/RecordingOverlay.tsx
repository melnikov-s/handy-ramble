import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
  PauseIcon,
  PlayIcon,
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
  | "paused"
  | "ramble_paused"
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
  // Mode determination state - hide pause button until mode is known
  const [modeKnown, setModeKnown] = useState(false);
  const [isQuickPressMode, setIsQuickPressMode] = useState(false);

  useEffect(() => {
    const setupEventListeners = async () => {
      // Listen for show-overlay event from Rust
      const unlistenShow = await listen("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();
        const overlayState = event.payload as string;
        console.log("Overlay state received:", overlayState);
        setState(overlayState as OverlayState);
        setErrorMessage("");
        setIsVisible(true);

        // Reset mode known state only when a NEW recording session starts (initial 'recording' state)
        // Do NOT reset when transitioning to 'ramble_recording' - that state is set AFTER mode is determined
        if (overlayState === "recording") {
          setModeKnown(false);
          setIsQuickPressMode(false);
        } else if (
          overlayState === "transcribing" ||
          overlayState === "ramble_transcribing" ||
          overlayState === "making_coherent"
        ) {
          // Reset when transitioning to processing states (new session will follow)
          setModeKnown(false);
          setIsQuickPressMode(false);
        }
        // Note: 'ramble_recording', 'paused', 'ramble_paused' do NOT reset mode
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
        // Reset mode state when hiding
        setModeKnown(false);
        setIsQuickPressMode(false);
      });

      // Listen for mode-determined event from Rust
      const unlistenMode = await listen<string>("mode-determined", (event) => {
        const mode = event.payload;
        setModeKnown(true);
        // 'refining' = quick press mode (toggle), pause button visible
        // 'hold' = PTT mode, no pause button
        setIsQuickPressMode(mode === "refining");
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

      // Listen for backend logs
      const unlistenLog = await listen<{ level: string; message: string }>(
        "backend-log",
        (event) => {
          const { level, message } = event.payload;
          if (level === "error") {
            console.error(`[Backend Error] ${message}`);
            // If it's a critical error, we might want to show it, but usually show-overlay-error handles that
          } else {
            console.log(`[Backend ${level}] ${message}`);
          }
        },
      );

      // Cleanup function
      return () => {
        unlistenShow();
        unlistenError();
        unlistenHide();
        unlistenMode();
        unlistenLevel();
        unlistenLog();
      };
    };

    setupEventListeners();
  }, []);

  // Auto-dismiss errors after 5 seconds
  useEffect(() => {
    if (state === "error" && isVisible) {
      const timer = setTimeout(() => {
        handleDismissError();
      }, 5000);
      return () => clearTimeout(timer);
    }
  }, [state, isVisible]);

  const handleDismissError = () => {
    setIsVisible(false);
    setErrorMessage("");
    setState("recording");
  };

  const handlePauseResume = () => {
    if (state === "paused" || state === "ramble_paused") {
      commands.resumeOperation();
    } else if (state === "recording" || state === "ramble_recording") {
      commands.pauseOperation();
    }
  };

  const isPaused = state === "paused" || state === "ramble_paused";
  const isRecording = state === "recording" || state === "ramble_recording";
  // Show pause button only when: mode is known AND in quick press mode (refining), OR already paused
  const showPauseButton =
    isPaused || (isRecording && modeKnown && isQuickPressMode);

  const getIcon = () => {
    if (state === "recording" || state === "ramble_recording") {
      return <MicrophoneIcon color="#1e40af" />;
    } else if (state === "making_coherent") {
      return <Sparkles size={16} style={{ color: "#1e40af" }} />;
    } else if (state === "ramble_transcribing" || state === "transcribing") {
      return <TranscriptionIcon color="#1e40af" />;
    } else if (state === "error") {
      return <AlertCircle size={16} style={{ color: "#ff6b6b" }} />;
    } else if (state === "paused" || state === "ramble_paused") {
      return <MicrophoneIcon color="#1e40af" />;
    } else {
      return <TranscriptionIcon color="#1e40af" />;
    }
  };

  return (
    <div
      className={`recording-overlay ${isVisible ? "fade-in" : ""} ${state === "error" ? "error-state" : ""} ${isPaused ? "paused-state" : ""}`}
    >
      <div className="overlay-left">{getIcon()}</div>

      <div className="overlay-middle">
        {(state === "recording" || state === "ramble_recording") && (
          <div className="stacked-content">
            {/* Only show label once mode is determined (after ~500ms threshold) */}
            {modeKnown && (
              <div
                className={`mode-label ${isQuickPressMode ? "refining-label" : "dictating-label"}`}
              >
                {isQuickPressMode
                  ? t("overlay.refined", "Refined")
                  : t("overlay.raw", "Raw")}
              </div>
            )}
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
        {isPaused && (
          <div className="stacked-content">
            <div className="mode-label paused-label">
              {t("overlay.paused", "Paused")}
            </div>
            <div className="bars-container paused-bars">
              {levels.map((_, i) => (
                <div key={i} className="bar paused-bar" />
              ))}
            </div>
          </div>
        )}
        {(state === "transcribing" || state === "ramble_transcribing") && (
          <div className="transcribing-text">{t("overlay.transcribing")}</div>
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
            {t("overlay.refinementFailed", "Refinement failed")}: {errorMessage}
          </div>
        )}
      </div>

      <div className="overlay-right">
        {(isRecording || isPaused) && (
          <>
            <div
              className="pause-button"
              onClick={handlePauseResume}
              style={{ visibility: showPauseButton ? "visible" : "hidden" }}
              title={
                isPaused
                  ? t("overlay.resume", "Resume")
                  : t("overlay.pause", "Pause")
              }
            >
              {isPaused ? (
                <PlayIcon width={16} height={16} color="#1e40af" />
              ) : (
                <PauseIcon width={16} height={16} color="#1e40af" />
              )}
            </div>
            <div
              className="cancel-button"
              onClick={() => {
                commands.cancelOperation();
              }}
            >
              <CancelIcon color="#1e40af" />
            </div>
          </>
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
