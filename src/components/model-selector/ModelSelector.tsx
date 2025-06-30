import React, { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ModelInfo } from "../../lib/types";

interface ModelStateEvent {
  event_type: string;
  model_id?: string;
  model_name?: string;
  error?: string;
}

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

type ModelStatus = "ready" | "loading" | "downloading" | "error" | "none";

interface DownloadStats {
  startTime: number;
  lastUpdate: number;
  totalDownloaded: number;
  speed: number;
}

interface ModelSelectorProps {
  onError?: (error: string) => void;
}

const ModelSelector: React.FC<ModelSelectorProps> = ({ onError }) => {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [currentModelId, setCurrentModelId] = useState<string>("");
  const [modelStatus, setModelStatus] = useState<ModelStatus>("loading");
  const [modelError, setModelError] = useState<string | null>(null);
  const [modelDownloadProgress, setModelDownloadProgress] = useState<
    Map<string, DownloadProgress>
  >(new Map());
  const [showModelDropdown, setShowModelDropdown] = useState(false);
  const [downloadStats, setDownloadStats] = useState<
    Map<string, DownloadStats>
  >(new Map());

  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadModels();
    loadCurrentModel();

    // Listen for model state changes
    const modelStateUnlisten = listen<ModelStateEvent>(
      "model-state-changed",
      (event) => {
        const { event_type, model_id, model_name, error } = event.payload;

        switch (event_type) {
          case "loading_started":
            setModelStatus("loading");
            setModelError(null);
            break;
          case "loading_completed":
            setModelStatus("ready");
            setModelError(null);
            if (model_id) setCurrentModelId(model_id);
            break;
          case "loading_failed":
            setModelStatus("error");
            setModelError(error || "Failed to load model");
            break;
        }
      },
    );

    // Listen for model download progress
    const downloadProgressUnlisten = listen<DownloadProgress>(
      "model-download-progress",
      (event) => {
        const progress = event.payload;
        setModelDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.set(progress.model_id, progress);
          return newMap;
        });
        setModelStatus("downloading");

        // Update download stats for speed calculation
        const now = Date.now();
        setDownloadStats((prev) => {
          const current = prev.get(progress.model_id);
          const newStats = new Map(prev);

          if (!current) {
            // First progress update - initialize
            newStats.set(progress.model_id, {
              startTime: now,
              lastUpdate: now,
              totalDownloaded: progress.downloaded,
              speed: 0,
            });
          } else {
            // Calculate speed over last few seconds
            const timeDiff = (now - current.lastUpdate) / 1000; // seconds
            const bytesDiff = progress.downloaded - current.totalDownloaded;

            if (timeDiff > 0.5) {
              // Update speed every 500ms
              const currentSpeed = bytesDiff / (1024 * 1024) / timeDiff; // MB/s
              // Smooth the speed with exponential moving average, but ensure positive values
              const validCurrentSpeed = Math.max(0, currentSpeed);
              const smoothedSpeed =
                current.speed > 0
                  ? current.speed * 0.8 + validCurrentSpeed * 0.2
                  : validCurrentSpeed;

              newStats.set(progress.model_id, {
                startTime: current.startTime,
                lastUpdate: now,
                totalDownloaded: progress.downloaded,
                speed: Math.max(0, smoothedSpeed),
              });
            }
          }

          return newStats;
        });
      },
    );

    // Listen for model download completion
    const downloadCompleteUnlisten = listen<string>(
      "model-download-complete",
      (event) => {
        const modelId = event.payload;
        setModelDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.delete(modelId);
          return newMap;
        });
        setDownloadStats((prev) => {
          const newStats = new Map(prev);
          newStats.delete(modelId);
          return newStats;
        });
        loadModels(); // Refresh models list

        // Auto-select the newly downloaded model
        setTimeout(() => {
          loadCurrentModel();
          handleModelSelect(modelId);
        }, 500);
      },
    );

    // Click outside to close dropdown
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setShowModelDropdown(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      modelStateUnlisten.then((fn) => fn());
      downloadProgressUnlisten.then((fn) => fn());
      downloadCompleteUnlisten.then((fn) => fn());
    };
  }, []);

  const loadModels = async () => {
    try {
      const modelList = await invoke<ModelInfo[]>("get_available_models");
      setModels(modelList);
    } catch (err) {
      console.error("Failed to load models:", err);
    }
  };

  const loadCurrentModel = async () => {
    try {
      const current = await invoke<string>("get_current_model");
      setCurrentModelId(current);

      if (current) {
        // Check if model is actually loaded
        const transcriptionStatus = await invoke<string | null>(
          "get_transcription_model_status",
        );
        if (transcriptionStatus === current) {
          setModelStatus("ready");
        } else {
          setModelStatus("loading");
        }
      } else {
        setModelStatus("none");
      }
    } catch (err) {
      console.error("Failed to load current model:", err);
      setModelStatus("error");
      setModelError("Failed to check model status");
    }
  };

  const handleModelSelect = async (modelId: string) => {
    try {
      setModelError(null);
      setShowModelDropdown(false);
      await invoke("set_active_model", { modelId });
      setCurrentModelId(modelId);
    } catch (err) {
      const errorMsg = `${err}`;
      setModelError(errorMsg);
      setModelStatus("error");
      onError?.(errorMsg);
    }
  };

  const handleModelDownload = async (modelId: string) => {
    try {
      setModelError(null);
      await invoke("download_model", { modelId });
    } catch (err) {
      const errorMsg = `${err}`;
      setModelError(errorMsg);
      setModelStatus("error");
      onError?.(errorMsg);
    }
  };

  const getModelStatusColor = (status: ModelStatus): string => {
    switch (status) {
      case "ready":
        return "bg-green-400";
      case "loading":
        return "bg-yellow-400 animate-pulse";
      case "downloading":
        return "bg-logo-primary animate-pulse";
      case "error":
        return "bg-red-400";
      case "none":
        return "bg-red-400";
      default:
        return "bg-mid-gray/60";
    }
  };

  const getCurrentModel = () => {
    return models.find((m) => m.id === currentModelId);
  };

  const getModelDisplayText = (): string => {
    if (modelDownloadProgress.size > 0) {
      if (modelDownloadProgress.size === 1) {
        const [progress] = Array.from(modelDownloadProgress.values());
        const percentage = Math.max(
          0,
          Math.min(100, Math.round(progress.percentage)),
        );
        return `Downloading ${percentage}%`;
      } else {
        return `Downloading ${modelDownloadProgress.size} models...`;
      }
    }

    const currentModel = getCurrentModel();

    switch (modelStatus) {
      case "ready":
        return currentModel?.name || "Model Ready";
      case "loading":
        return currentModel ? `Loading ${currentModel.name}...` : "Loading...";
      case "error":
        return modelError || "Model Error";
      case "none":
        return "No Model - Download Required";
      default:
        return currentModel?.name || "Select Model";
    }
  };

  const formatFileSize = (sizeMb: number): string => {
    if (sizeMb < 1024) return `${sizeMb}MB`;
    return `${(sizeMb / 1024).toFixed(1)}GB`;
  };

  const getAvailableModels = () => {
    return models.filter((m) => m.is_downloaded);
  };

  const getDownloadableModels = () => {
    return models.filter((m) => !m.is_downloaded);
  };

  const availableModels = getAvailableModels();
  const downloadableModels = getDownloadableModels();
  const isFirstRun = availableModels.length === 0 && models.length > 0;

  return (
    <>
      {/* Model Status and Switcher */}
      <div className="relative" ref={dropdownRef}>
        <button
          onClick={() => setShowModelDropdown(!showModelDropdown)}
          className="flex items-center gap-2 hover:text-text/80 transition-colors"
          title={`Model status: ${getModelDisplayText()}`}
        >
          <div
            className={`w-2 h-2 rounded-full ${getModelStatusColor(modelStatus)}`}
          />
          <span className="max-w-28 truncate">{getModelDisplayText()}</span>
          <svg
            className={`w-3 h-3 transition-transform ${showModelDropdown ? "rotate-180" : ""}`}
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

        {/* Model Dropdown */}
        {showModelDropdown && (
          <div className="absolute bottom-full left-0 mb-2 w-64 bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50">
            {/* First Run Welcome */}
            {isFirstRun && (
              <div className="px-3 py-2 bg-logo-primary/10 border-b border-logo-primary/20">
                <div className="text-xs font-medium text-logo-primary mb-1">
                  Welcome to Handy!
                </div>
                <div className="text-xs text-text/70">
                  Download a model below to get started with transcription.
                </div>
              </div>
            )}

            {/* Available Models */}
            {availableModels.length > 0 && (
              <div>
                <div className="px-3 py-1 text-xs font-medium text-text/80 border-b border-mid-gray/10">
                  Available Models
                </div>
                {availableModels.map((model) => (
                  <div
                    key={model.id}
                    onClick={() => handleModelSelect(model.id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleModelSelect(model.id);
                      }
                    }}
                    tabIndex={0}
                    role="button"
                    className={`w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                      currentModelId === model.id
                        ? "bg-logo-primary/10 text-logo-primary"
                        : ""
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div>
                        <div className="text-sm">{model.name}</div>
                        <div className="text-xs text-text/40 italic pr-4">
                          {model.description}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        {currentModelId === model.id && (
                          <div className="text-xs text-logo-primary">
                            Active
                          </div>
                        )}
                        {(() => {
                          const shouldShowDelete = currentModelId !== model.id;
                          return shouldShowDelete;
                        })() && (
                          <button
                            onClick={async (e) => {
                              e.preventDefault();
                              e.stopPropagation();
                              console.log(
                                "Delete button clicked for model:",
                                model.id,
                              );
                              console.log("Model info:", model);

                              try {
                                console.log("Deleting model:", model.id);

                                const result = await invoke("delete_model", {
                                  modelId: model.id,
                                });

                                console.log("Delete result:", result);

                                // Refresh models list but keep dropdown open
                                await loadModels();
                                setModelError(null);

                                console.log("Model deleted and UI refreshed");
                              } catch (err) {
                                console.error("Delete failed with error:", err);
                                const errorMsg = `Failed to delete model: ${err}`;
                                setModelError(errorMsg);
                                setModelStatus("error");
                                onError?.(errorMsg);
                              }
                            }}
                            className="text-red-400 hover:text-red-300 p-1 hover:bg-red-500/10 rounded transition-colors"
                            title={`Delete ${model.name}`}
                          >
                            <svg
                              className="w-3 h-3"
                              fill="currentColor"
                              viewBox="0 0 20 20"
                            >
                              <path
                                fillRule="evenodd"
                                d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z"
                                clipRule="evenodd"
                              />
                            </svg>
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* Downloadable Models */}
            {downloadableModels.length > 0 && (
              <div>
                {(availableModels.length > 0 || isFirstRun) && (
                  <div className="border-t border-mid-gray/10 my-1" />
                )}
                <div className="px-3 py-1 text-xs font-medium text-text/80">
                  {isFirstRun ? "Choose a Model" : "Download Models"}
                </div>
                {downloadableModels.map((model) => (
                  <div
                    key={model.id}
                    onClick={(e) => {
                      if (modelDownloadProgress.has(model.id)) {
                        e.preventDefault();
                        return;
                      }
                      handleModelDownload(model.id);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        if (modelDownloadProgress.has(model.id)) {
                          return;
                        }
                        handleModelDownload(model.id);
                      }
                    }}
                    tabIndex={0}
                    role="button"
                    aria-disabled={modelDownloadProgress.has(model.id)}
                    className={`w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                      modelDownloadProgress.has(model.id)
                        ? "opacity-50 cursor-not-allowed hover:bg-transparent"
                        : ""
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div>
                        <div className="text-sm">
                          {model.name}
                          {model.id === "small" && isFirstRun && (
                            <span className="ml-2 text-xs bg-logo-primary/20 text-logo-primary px-1.5 py-0.5 rounded">
                              Recommended
                            </span>
                          )}
                        </div>
                        <div className="text-xs text-text/40 italic pr-4">
                          {model.description}
                        </div>
                      </div>
                      {modelDownloadProgress.has(model.id) ? (
                        <div className="text-xs text-logo-primary tabular-nums">
                          {Math.max(
                            0,
                            Math.min(
                              100,
                              Math.round(
                                modelDownloadProgress.get(model.id)!.percentage,
                              ),
                            ),
                          )}
                          %
                        </div>
                      ) : (
                        <div className="text-xs text-logo-primary">
                          Download
                        </div>
                      )}
                    </div>

                    {modelDownloadProgress.has(model.id) && (
                      <div className="mt-2">
                        <div className="w-full bg-mid-gray/20 rounded-full h-1.5">
                          <div
                            className="h-1.5 bg-logo-primary rounded-full transition-all duration-500 ease-out"
                            style={{
                              width: `${Math.max(0, Math.min(100, modelDownloadProgress.get(model.id)!.percentage))}%`,
                            }}
                          />
                        </div>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}

            {/* No Models Available */}
            {availableModels.length === 0 &&
              downloadableModels.length === 0 && (
                <div className="px-3 py-2 text-sm text-text/60">
                  No models available
                </div>
              )}
          </div>
        )}
      </div>

      {/* Download Progress Bar for Models */}
      {modelDownloadProgress.size > 0 && (
        <div className="flex items-center gap-3">
          {modelDownloadProgress.size === 1 ? (
            // Single download - show detailed progress
            (() => {
              const [progress] = Array.from(modelDownloadProgress.values());
              return (
                <>
                  <div className="w-16 h-1.5 bg-mid-gray/20 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-logo-primary transition-all duration-500 ease-out"
                      style={{
                        width: `${Math.max(0, Math.min(100, progress.percentage))}%`,
                      }}
                    />
                  </div>
                  <div className="text-xs text-text/60 tabular-nums min-w-fit">
                    {downloadStats.get(progress.model_id)?.speed &&
                    downloadStats.get(progress.model_id)!.speed > 0 ? (
                      <span>
                        {downloadStats.get(progress.model_id)!.speed.toFixed(1)}
                        MB/s
                      </span>
                    ) : (
                      <span>Downloading...</span>
                    )}
                  </div>
                </>
              );
            })()
          ) : (
            // Multiple downloads - show summary
            <div className="flex items-center gap-2">
              <div className="flex gap-1">
                {Array.from(modelDownloadProgress.values()).map(
                  (progress, index) => (
                    <div
                      key={progress.model_id}
                      className="w-3 h-1.5 bg-mid-gray/20 rounded-full overflow-hidden"
                    >
                      <div
                        className="h-full bg-logo-primary transition-all duration-500 ease-out"
                        style={{
                          width: `${Math.max(0, Math.min(100, progress.percentage))}%`,
                        }}
                      />
                    </div>
                  ),
                )}
              </div>
              <div className="text-xs text-text/60 min-w-fit">
                {modelDownloadProgress.size} downloading...
              </div>
            </div>
          )}
        </div>
      )}
    </>
  );
};

export default ModelSelector;
