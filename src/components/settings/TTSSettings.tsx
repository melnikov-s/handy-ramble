import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { RambleShortcut } from "./RambleShortcut";
import { commands, ModelInfo } from "@/bindings";
import { Download, Check, Loader2 } from "lucide-react";

export const TTSSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);

  useEffect(() => {
    const loadModels = async () => {
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        setAvailableModels(result.data.filter((m) => m.engine_type === "TTS"));
      }
    };
    loadModels();
  }, []);

  const handleModelDownload = async (modelId: string) => {
    try {
      await commands.downloadModel(modelId);
      // Refresh models after download trigger
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        setAvailableModels(result.data.filter((m) => m.engine_type === "TTS"));
      }
    } catch (error) {
      console.error("Failed to download model:", error);
    }
  };

  const tts_enabled = settings?.tts_enabled ?? true;
  const selectedModelId = settings?.tts_selected_model || "kokoro-82m";
  const selectedModel = availableModels.find((m) => m.id === selectedModelId);

  return (
    <SettingsGroup title={t("settings.tts.title", "Text to Speech")}>
      <RambleShortcut shortcutId="speak_selection" grouped={true} />

      <ToggleSwitch
        label={t("settings.tts.enabled.title", "Enable AI Speech")}
        description={t(
          "settings.tts.enabled.description",
          "Speak selected text aloud using high-quality local AI models.",
        )}
        checked={tts_enabled}
        onChange={(checked: boolean) => updateSetting("tts_enabled", checked)}
        isUpdating={isUpdating("tts_enabled")}
        grouped={true}
      />

      <SettingContainer
        title={t("settings.tts.model.title", "Speech Model")}
        description={t(
          "settings.tts.model.description",
          "Choose the AI voice model. Kokoro is recommended for natural sound.",
        )}
        layout="horizontal"
        grouped={true}
      >
        <div className="flex flex-col gap-2 min-w-[200px]">
          <select
            value={selectedModelId}
            onChange={(e) =>
              updateSetting("tts_selected_model", e.target.value)
            }
            className="px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
          >
            {availableModels.map((model) => (
              <option key={model.id} value={model.id}>
                {model.name} ({model.size_mb}MB)
              </option>
            ))}
          </select>

          {selectedModel && !selectedModel.is_downloaded && (
            <button
              onClick={() => handleModelDownload(selectedModel.id)}
              disabled={selectedModel.is_downloading}
              className="flex items-center justify-center gap-2 px-3 py-1.5 bg-logo-primary/10 text-logo-primary hover:bg-logo-primary/20 rounded-lg text-xs transition-colors disabled:opacity-50"
            >
              {selectedModel.is_downloading ? (
                <>
                  <Loader2 className="h-3 w-3 animate-spin" />
                  {t("common.downloading", "Downloading...")}
                </>
              ) : (
                <>
                  <Download className="h-3 w-3" />
                  {t("settings.tts.download", "Download Model")}
                </>
              )}
            </button>
          )}
          {selectedModel?.is_downloaded && (
            <div className="flex items-center gap-1 text-[10px] text-green-500 justify-end">
              <Check className="h-3 w-3" />
              {t("settings.tts.ready", "Ready for local use")}
            </div>
          )}
        </div>
      </SettingContainer>

      <SettingContainer
        title={t("settings.tts.speed.title", "Speech Speed")}
        description={t(
          "settings.tts.speed.description",
          "Adjust how fast the AI speaks.",
        )}
        layout="stacked"
        grouped={true}
      >
        <div className="flex items-center gap-4 w-full py-2">
          <input
            type="range"
            min="0.5"
            max="2.0"
            step="0.1"
            value={settings?.tts_speed ?? 1.0}
            onChange={(e) =>
              updateSetting("tts_speed", parseFloat(e.target.value))
            }
            className="flex-1 accent-logo-primary h-1 bg-mid-gray/20 rounded-lg appearance-none cursor-pointer"
          />
          <span className="text-xs font-mono w-8 text-right">
            {(settings?.tts_speed ?? 1.0).toFixed(1)}x
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t("settings.tts.volume.title", "Speech Volume")}
        description={t(
          "settings.tts.volume.description",
          "Adjust the playback volume of the AI voice.",
        )}
        layout="stacked"
        grouped={true}
      >
        <div className="flex items-center gap-4 w-full py-2">
          <input
            type="range"
            min="0.0"
            max="1.0"
            step="0.05"
            value={settings?.tts_volume ?? 1.0}
            onChange={(e) =>
              updateSetting("tts_volume", parseFloat(e.target.value))
            }
            className="flex-1 accent-logo-primary h-1 bg-mid-gray/20 rounded-lg appearance-none cursor-pointer"
          />
          <span className="text-xs font-mono w-8 text-right">
            {Math.round((settings?.tts_volume ?? 1.0) * 100)}%
          </span>
        </div>
      </SettingContainer>
    </SettingsGroup>
  );
};
