import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
  PauseIcon,
  PlayIcon,
} from "../components/icons";
import { Sparkles, AlertCircle, X, Camera, FileText } from "lucide-react";
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

// Prompt mode type matches Rust PromptMode enum
type PromptMode =
  | "dynamic"
  | "development"
  | "conversation"
  | "writing"
  | "email";

interface ErrorPayload {
  state: string;
  message: string;
}

// Icons for prompt modes (emoji for most, null for dynamic which shows detected category)
const PROMPT_MODE_ICONS: Record<PromptMode, string | null> = {
  dynamic: null, // Uses detected category icon or ear icon
  development: "💻",
  conversation: "💬",
  writing: "✍️",
  email: "📧",
};

// Icons for category IDs (used in Dynamic mode to show detected category)
const CATEGORY_ICONS: Record<string, string> = {
  development: "💻",
  conversation: "💬",
  writing: "✍️",
  email: "📧",
};

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
  const [flashScreenshot, setFlashScreenshot] = useState(false);
  const [hasScreenshot, setHasScreenshot] = useState(false);
  // Prompt mode state (from tray menu selection)
  const [promptMode, setPromptMode] = useState<PromptMode>("dynamic");
  // Detected category in Dynamic mode (from backend when refinement starts)
  const [detectedCategory, setDetectedCategory] = useState<string | null>(null);

  // Track pending optimistic flashes to prevent duplicates from backend events
  const pendingOptimisticFlashesRef = useRef(0);

  // Context params count (for badge)
  const [contextParamsCount, setContextParamsCount] = useState(0);

  useEffect(() => {
    let isMounted = true;
    const unlisteners: (() => void)[] = [];

    const setupEventListeners = async () => {
      // Helper to safely register listeners
      const register = async <T,>(
        event: string,
        handler: (event: { payload: T }) => void,
      ) => {
        const unlisten = await listen<T>(event, handler);
        if (!isMounted) {
          unlisten();
        } else {
          unlisteners.push(unlisten);
        }
      };

      // Listen for show-overlay event from Rust
      await register<string>("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();
        const overlayState = event.payload;
        console.log("[UI] show-overlay received:", overlayState);
        setState(overlayState as OverlayState);
        setErrorMessage("");
        setIsVisible(true);

        // Fetch current prompt mode from settings
        try {
          const settings = await commands.getAppSettings();
          if (settings.status === "ok") {
            // Cast to any until TypeScript bindings are regenerated
            const data = settings.data as any;
            if (data.prompt_mode) {
              setPromptMode(data.prompt_mode as PromptMode);
            }
          }
        } catch (e) {
          console.error("Failed to fetch prompt mode:", e);
        }

        // Reset mode known state only when a NEW recording session starts (initial 'recording' state)
        if (overlayState === "recording") {
          setModeKnown(false);
          setIsQuickPressMode(false);
          setHasScreenshot(false);
          setDetectedCategory(null);
          pendingOptimisticFlashesRef.current = 0;
        } else if (
          overlayState === "transcribing" ||
          overlayState === "ramble_transcribing" ||
          overlayState === "making_coherent"
        ) {
          setModeKnown(false);
          setIsQuickPressMode(false);
        }
      });

      // Listen for prompt mode changes from tray menu
      await register<PromptMode>("prompt-mode-changed", (event) => {
        console.log("[UI] prompt-mode-changed received:", event.payload);
        setPromptMode(event.payload);
      });

      // Listen for detected category in Dynamic mode
      await register<string>("category-detected", (event) => {
        console.log("[UI] category-detected received:", event.payload);
        setDetectedCategory(event.payload);
      });

      // Listen for error overlay event from Rust
      await register<ErrorPayload>("show-overlay-error", async (event) => {
        await syncLanguageFromSettings();
        setState("error");
        setErrorMessage(event.payload.message);
        setIsVisible(true);
      });

      // Listen for hide-overlay event from Rust
      await register<void>("hide-overlay", () => {
        setIsVisible(false);
        setErrorMessage("");
        setModeKnown(false);
        setIsQuickPressMode(false);
        setHasScreenshot(false);
        setDetectedCategory(null);
        pendingOptimisticFlashesRef.current = 0;
      });

      // Listen for mode-determined event from Rust
      await register<string>("mode-determined", (event) => {
        const mode = event.payload;
        console.log("[UI] mode-determined received:", mode);
        setModeKnown(true);
        setIsQuickPressMode(mode === "refining");
      });

      // Listen for mic-level updates
      await register<number[]>("mic-level", (event) => {
        const newLevels = event.payload;
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3;
        });
        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 9));
      });

      // Listen for backend logs
      await register<{ level: string; message: string }>(
        "backend-log",
        (event) => {
          const { level, message } = event.payload;
          if (level === "error") {
            console.error(`[Backend Error] ${message}`);
          } else {
            console.log(`[Backend ${level}] ${message}`);
          }
        },
      );

      // Listen for vision capture feedback from Rust
      await register<void>("vision-captured", () => {
        console.log("[UI] vision-captured received");
        setHasScreenshot(true);

        if (pendingOptimisticFlashesRef.current > 0) {
          console.log(
            "[UI] Suppressing duplicate flash (handled optimistically)",
          );
          pendingOptimisticFlashesRef.current -= 1;
          return;
        }

        setFlashScreenshot(true);
        setTimeout(() => setFlashScreenshot(false), 500);
      });
    };

    setupEventListeners();

    return () => {
      isMounted = false;
      unlisteners.forEach((u) => u());
    };
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

  const handleVisionCapture = () => {
    // Optimistically trigger flash animation
    // Increment counter so we ignore the subsequent backend confirmation
    pendingOptimisticFlashesRef.current += 1;
    setHasScreenshot(true);
    setFlashScreenshot(true);
    setTimeout(() => setFlashScreenshot(false), 500);

    // Trigger backend command
    invoke("trigger_vision_capture").catch((err) =>
      console.error("Failed to trigger vision capture:", err),
    );
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

  // Show vision indicator if: recording or paused, preventing it from showing during processing
  const isProcessing =
    state === "transcribing" ||
    state === "ramble_transcribing" ||
    state === "making_coherent" ||
    state === "error";

  // Only show vision button when in "Refined" (quick press) mode, or if we already have a screenshot attached.
  // In "Raw" mode (hold), screenshots are not used, so we hide the button to avoid confusion.
  // We also check modeKnown to avoid showing it prematurely before we know if it's Raw or Refined.
  const showVisionButton =
    !isProcessing &&
    (isRecording || isPaused) &&
    ((modeKnown && isQuickPressMode) || hasScreenshot);

  const getIcon = () => {
    // Helper to get category icon
    const getCategoryIcon = () => {
      // For explicit modes, use the mode's icon
      if (promptMode !== "dynamic") {
        return (
          <span className="prompt-mode-icon-main">
            {PROMPT_MODE_ICONS[promptMode]}
          </span>
        );
      }
      // For Dynamic mode, show detected category icon if available
      if (detectedCategory && CATEGORY_ICONS[detectedCategory]) {
        return (
          <span className="prompt-mode-icon-main">
            {CATEGORY_ICONS[detectedCategory]}
          </span>
        );
      }
      // Fallback to microphone icon
      return <MicrophoneIcon color="#1e40af" />;
    };

    if (state === "recording" || state === "ramble_recording") {
      // In Refined mode, show category icon
      if (modeKnown && isQuickPressMode) {
        return getCategoryIcon();
      }
      return <MicrophoneIcon color="#1e40af" />;
    } else if (state === "making_coherent") {
      // While refining, show the detected category icon
      return getCategoryIcon();
    } else if (state === "ramble_transcribing" || state === "transcribing") {
      return <TranscriptionIcon color="#1e40af" />;
    } else if (state === "error") {
      return <AlertCircle size={16} style={{ color: "#ff6b6b" }} />;
    } else if (state === "paused" || state === "ramble_paused") {
      // In Refined paused mode, show category icon
      if (isQuickPressMode) {
        return getCategoryIcon();
      }
      return <MicrophoneIcon color="#1e40af" />;
    } else {
      return <TranscriptionIcon color="#1e40af" />;
    }
  };

  return (
    <>
      <div
        className={`recording-overlay ${isVisible ? "fade-in" : ""} ${state === "error" ? "error-state" : ""} ${isPaused ? "paused-state" : ""} ${flashScreenshot ? "screenshot-flash" : ""}`}
      >
        <div className="overlay-left">
          {getIcon()}
          {/* Show vision indicator if enabled */}
          {showVisionButton && (
            <div
              className={`vision-indicator ${hasScreenshot ? "has-vision" : ""}`}
              style={{
                opacity: isQuickPressMode ? 1 : 0.4,
                cursor: "pointer",
                pointerEvents: "auto",
              }}
              onClick={handleVisionCapture}
              title={
                hasScreenshot
                  ? t("overlay.visionCaptured", "Screenshot taken")
                  : t("overlay.takeVision", "Click or Press S for screenshot")
              }
            >
              <Camera size={14} />
            </div>
          )}
        </div>

        <div className="overlay-middle">
          {(state === "recording" || state === "ramble_recording") && (
            <div className="stacked-content">
              <div className="bars-container">
                {levels.map((v, i) => (
                  <div
                    key={i}
                    className="bar"
                    style={{
                      height: `${Math.min(14, 3 + Math.pow(v, 0.7) * 11)}px`,
                      transition:
                        "height 60ms ease-out, opacity 120ms ease-out",
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
              {t("overlay.refinementFailed", "Refinement failed")}:{" "}
              {errorMessage}
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
    </>
  );
};

export default RecordingOverlay;
