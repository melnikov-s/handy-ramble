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
import { AlertCircle, X, Camera } from "lucide-react";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import { syncLanguageFromSettings } from "@/i18n";

type OverlayState =
  | "recording"
  | "ramble_recording"
  | "voice_command_recording"
  | "context_chat_recording"
  | "transcribing"
  | "voice_command_transcribing"
  | "context_chat_processing"
  | "making_coherent"
  | "processing_command"
  | "paused"
  | "ramble_paused"
  | "speaking"
  | "error";

// Prompt mode type matches Rust PromptMode enum
type PromptMode = "dynamic" | "low" | "medium" | "high";

interface ErrorPayload {
  state: string;
  message: string;
  is_voice_command: boolean;
}

// Icons for prompt modes (bar characters for intensity, null for dynamic which shows detected category)
const PROMPT_MODE_ICONS: Record<PromptMode, string | null> = {
  dynamic: null, // Uses detected category icon or ear icon
  low: "▁",
  medium: "▃",
  high: "▅",
};

// Icons for category IDs (used in Dynamic mode to show detected category)
const CATEGORY_ICONS: Record<string, string> = {
  low: "▁",
  medium: "▃",
  high: "▅",
};

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [errorMessage, setErrorMessage] = useState<string>("");
  const [isVoiceCommandError, setIsVoiceCommandError] = useState(false);
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  // Mode determination state - hide pause button until mode is known
  const [modeKnown, setModeKnown] = useState(false);
  const [isQuickPressMode, setIsQuickPressMode] = useState(false);

  // Prompt mode state (from tray menu selection)
  const [promptMode, setPromptMode] = useState<PromptMode>("dynamic");
  // Detected category in Dynamic mode (from backend when refinement starts)
  const [detectedCategory, setDetectedCategory] = useState<string | null>(null);

  // Helper to determine if current state is voice command related (purple theme)
  const isVoiceCommandState =
    state === "voice_command_recording" ||
    state === "voice_command_transcribing" ||
    state === "processing_command";

  // Helper to determine if current state is context chat related (gold theme)
  const isContextChatState =
    state === "context_chat_recording" || state === "context_chat_processing";

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

      // Listen for vision-captured event to update counter
      await register<void>("vision-captured", () => {
        setContextParamsCount((prev) => prev + 1);
      });

      // Listen for show-overlay event from Rust
      await register<string>("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();
        const overlayState = event.payload;
        console.log("[UI] show-overlay received:", overlayState);
        setState(overlayState as OverlayState);
        setErrorMessage("");
        setIsVoiceCommandError(false);
        setIsVisible(true);

        // Reset context count on new session
        if (
          overlayState === "recording" ||
          overlayState === "voice_command_recording" ||
          overlayState === "context_chat_recording"
        ) {
          setContextParamsCount(0);
        }

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

        // Reset mode known state only when a NEW recording session starts
        if (
          overlayState === "recording" ||
          overlayState === "voice_command_recording" ||
          overlayState === "context_chat_recording"
        ) {
          setModeKnown(false);
          setIsQuickPressMode(false);
          setDetectedCategory(null);
        } else if (
          overlayState === "transcribing" ||
          overlayState === "ramble_transcribing" ||
          overlayState === "voice_command_transcribing" ||
          overlayState === "context_chat_processing" ||
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
        setIsVoiceCommandError(event.payload.is_voice_command);
        setIsVisible(true);
      });

      // Listen for hide-overlay event from Rust
      await register<void>("hide-overlay", () => {
        setIsVisible(false);
        setErrorMessage("");
        setModeKnown(false);
        setIsQuickPressMode(false);
        setDetectedCategory(null);
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

      // Note: processing-command state is now set via show-overlay event from backend
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
    setIsVoiceCommandError(false);
    setState("recording");
  };

  const handleScreenshot = async () => {
    try {
      const result = await commands.captureScreenMode(false);
      if (result.status === "ok") {
        await commands.addContextImage(result.data);
        // Visual feedback - flash the overlay
        const overlay = document.querySelector(".recording-overlay");
        if (overlay) {
          overlay.classList.add("screenshot-flash");
          setTimeout(() => overlay.classList.remove("screenshot-flash"), 300);
        }
      }
    } catch (e) {
      console.error("Failed to capture screenshot:", e);
    }
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
    // Helper to get category icon (only for refiner mode, not voice commands)
    const getCategoryIcon = () => {
      // In voice command mode, always use microphone - no category needed
      if (isVoiceCommandState) {
        return <MicrophoneIcon color="#a855f7" />;
      }
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

    if (state === "voice_command_recording") {
      // Voice command recording - always purple microphone, no category icons
      return <MicrophoneIcon color="#a855f7" />;
    } else if (state === "context_chat_recording") {
      // Context chat recording - always gold microphone
      return <MicrophoneIcon color="#f59e0b" />;
    } else if (state === "recording" || state === "ramble_recording") {
      // In Refined mode, show category icon
      if (modeKnown && isQuickPressMode) {
        return getCategoryIcon();
      }
      return <MicrophoneIcon color="#1e40af" />;
    } else if (state === "making_coherent") {
      // While refining, show the detected category icon
      return getCategoryIcon();
    } else if (
      state === "processing_command" ||
      state === "context_chat_processing"
    ) {
      // Processing voice command or context chat
      return (
        <MicrophoneIcon color={isContextChatState ? "#f59e0b" : "#a855f7"} />
      );
    } else if (
      state === "transcribing" ||
      state === "voice_command_transcribing"
    ) {
      return (
        <TranscriptionIcon
          color={isVoiceCommandState ? "#a855f7" : "#1e40af"}
        />
      );
    } else if (state === "error") {
      return <AlertCircle size={16} style={{ color: "#ff6b6b" }} />;
    } else if (state === "paused" || state === "ramble_paused") {
      // In Refined paused mode, show category icon
      if (isQuickPressMode) {
        return getCategoryIcon();
      }
      return <MicrophoneIcon color="#1e40af" />;
    } else if (state === "speaking") {
      // Speaking state - show speaker/audio icon
      return <span className="speaking-icon">🔊</span>;
    } else {
      return <TranscriptionIcon color="#1e40af" />;
    }
  };

  return (
    <>
      <div
        className={`recording-overlay bg-app-base ${isVisible ? "fade-in" : ""} ${state === "error" ? "error-state" : ""} ${isPaused ? "paused-state" : ""} ${isVoiceCommandState ? "voice-command-mode" : ""} ${isContextChatState ? "context-chat-mode" : ""}`}
      >
        <div className="overlay-left">{getIcon()}</div>

        <div className="overlay-middle">
          {(state === "recording" ||
            state === "ramble_recording" ||
            state === "voice_command_recording" ||
            state === "context_chat_recording") && (
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
          {(state === "transcribing" ||
            state === "voice_command_transcribing") && (
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
          {(state === "processing_command" ||
            state === "context_chat_processing") && (
            <div className="stacked-content">
              <div className="mode-label refining-label">
                {t("overlay.processingCommand", "Processing...")}
              </div>
              <div className="refining-indicator">
                <div className="refining-dot"></div>
                <div className="refining-dot"></div>
                <div className="refining-dot"></div>
              </div>
            </div>
          )}
          {state === "speaking" && (
            <div className="stacked-content">
              <div className="mode-label speaking-label">
                {t("overlay.speaking", "Speaking...")}
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
              {isVoiceCommandError
                ? t("overlay.commandFailed", "Command failed")
                : t("overlay.refinementFailed", "Refinement failed")}
              : {errorMessage}
            </div>
          )}
        </div>

        <div className="overlay-right">
          {isContextChatState && state === "context_chat_recording" && (
            <div
              className={`vision-indicator ${contextParamsCount > 0 ? "has-vision" : ""}`}
              onClick={handleScreenshot}
              title={t("overlay.screenshot", "Capture Screenshot (S)")}
            >
              <Camera size={16} />
            </div>
          )}
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
                  <PlayIcon
                    width={16}
                    height={16}
                    color={isContextChatState ? "#f59e0b" : "#1e40af"}
                  />
                ) : (
                  <PauseIcon
                    width={16}
                    height={16}
                    color={isContextChatState ? "#f59e0b" : "#1e40af"}
                  />
                )}
              </div>
              <div
                className="cancel-button"
                onClick={() => {
                  commands.cancelOperation();
                }}
              >
                <CancelIcon
                  color={
                    isContextChatState
                      ? "#f59e0b"
                      : isVoiceCommandState
                        ? "#a855f7"
                        : "#1e40af"
                  }
                />
              </div>
            </>
          )}
          {state === "speaking" && (
            <div
              className="cancel-button"
              onClick={() => {
                commands.cancelOperation();
              }}
              title={t("overlay.stopSpeaking", "Stop (Esc)")}
            >
              <CancelIcon color="#1e40af" />
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
    </>
  );
};

export default RecordingOverlay;
