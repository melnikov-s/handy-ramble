import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Download, Check, Loader2 } from "lucide-react";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { ModelsDropdown } from "../ui/ModelsDropdown";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { commands, ModelInfo } from "@/bindings";
import { RambleShortcut } from "./RambleShortcut";
import { LanguageSelector } from "./LanguageSelector";

export const TextToSpeechSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, refreshSettings, isUpdating } =
    useSettings();
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
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        setAvailableModels(result.data.filter((m) => m.engine_type === "TTS"));
      }
    } catch (error) {
      console.error("Failed to download model:", error);
    }
  };

  const handleContextChatModelChange = async (modelId: string | null) => {
    if (!modelId) return;
    try {
      await commands.setDefaultModel("context_chat", modelId);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change context chat model:", error);
    }
  };

  const tts_enabled = settings?.tts_enabled ?? true;
  const ttsSelectedModelId = settings?.tts_selected_model || "kokoro-82m";
  const ttsSelectedModel = availableModels.find(
    (m) => m.id === ttsSelectedModelId,
  );
  const contextChatModelId = settings?.default_context_chat_model_id || null;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("sidebar.textToSpeech", "Text to Speech")}>
        <RambleShortcut shortcutId="speak_selection" grouped={true} />
        <RambleShortcut shortcutId="context_chat" grouped={true} />
        <LanguageSelector descriptionMode="tooltip" grouped={true} />

        <ToggleSwitch
          label="Enable AI Speech"
          description="Speak selected text aloud using high-quality local AI models."
          checked={tts_enabled}
          onChange={(checked: boolean) => updateSetting("tts_enabled", checked)}
          isUpdating={isUpdating("tts_enabled")}
          grouped={true}
        />

        <SettingContainer
          title="Speech Model"
          description="Choose the AI voice model. Kokoro is recommended for natural sound."
          layout="horizontal"
          grouped={true}
        >
          <div className="flex flex-col gap-2 min-w-[200px]">
            <select
              value={ttsSelectedModelId}
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

            {ttsSelectedModel && !ttsSelectedModel.is_downloaded && (
              <button
                onClick={() => handleModelDownload(ttsSelectedModel.id)}
                disabled={ttsSelectedModel.is_downloading}
                className="flex items-center justify-center gap-2 px-3 py-1.5 bg-logo-primary/10 text-logo-primary hover:bg-logo-primary/20 rounded-lg text-xs transition-colors disabled:opacity-50"
              >
                {ttsSelectedModel.is_downloading ? (
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
            {ttsSelectedModel?.is_downloaded && (
              <div className="flex items-center gap-1 text-[10px] text-green-500 justify-end">
                <Check className="h-3 w-3" />
                {t("settings.tts.ready", "Ready for local use")}
              </div>
            )}
          </div>
        </SettingContainer>

        <SettingContainer
          title="Speech Speed"
          description="Adjust how fast the AI speaks."
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
          title="Speech Volume"
          description="Adjust the playback volume of the AI voice."
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

      <SettingsGroup title="Voice Interaction">
        <SettingContainer
          title="Chat Model"
          description="The AI model used for context-aware voice chat. Vision-capable models are recommended if you use screenshots."
          descriptionMode="tooltip"
          layout="stacked"
          grouped={true}
        >
          <ModelsDropdown
            selectedValue={contextChatModelId}
            onSelect={handleContextChatModelChange}
            allowDefault={true}
            defaultLabel="Use Default (Chat Model)"
            className="min-w-[380px]"
          />
        </SettingContainer>

        <SettingContainer
          title="Interaction Prompt"
          description="Customize how the AI interprets your context and spoken question."
          descriptionMode="tooltip"
          layout="stacked"
          grouped={true}
        >
          <textarea
            value={settings?.context_chat_prompt ?? ""}
            onChange={async (e) => {
              try {
                await commands.changeContextChatPromptSetting(e.target.value);
                await refreshSettings();
              } catch (error) {
                console.error("Failed to update context chat prompt:", error);
              }
            }}
            placeholder="Enter prompt template..."
            className="w-full min-h-[150px] p-2 bg-mid-gray/5 border border-mid-gray/20 rounded text-sm focus:outline-none focus:border-logo-primary resize-y font-mono"
          />
          <div className="mt-2 text-xs text-mid-gray">
            Available variables: <code>{"${selection}"}</code>,{" "}
            <code>{"${clipboard}"}</code>, <code>{"${prompt}"}</code> (your
            speech)
          </div>
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
