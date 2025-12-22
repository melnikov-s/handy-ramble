import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCcw, RotateCcw } from "lucide-react";
import { commands } from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { ResetButton } from "../ui/ResetButton";

import { ProviderSelect } from "./PostProcessingSettingsApi/ProviderSelect";
import { ApiKeyField } from "./PostProcessingSettingsApi/ApiKeyField";
import { ModelSelect } from "./PostProcessingSettingsApi/ModelSelect";
import { useSettings } from "../../hooks/useSettings";
import { ToggleSwitch } from "../ui/ToggleSwitch";

export const RambleSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();

  const [prompt, setPrompt] = useState("");

  // Default Gemini models for pre-population
  const defaultGeminiModels = [
    {
      value: "gemini-2.5-flash-lite",
      label: "Gemini 2.5 Flash Lite (Fastest)",
    },
    { value: "gemini-2.0-flash", label: "Gemini 2.0 Flash" },
    { value: "gemini-2.0-flash-lite", label: "Gemini 2.0 Flash Lite" },
    {
      value: "gemini-3-flash-preview",
      label: "Gemini 3 Flash Preview (Thinking)",
    },
    { value: "gemini-1.5-flash", label: "Gemini 1.5 Flash" },
    { value: "gemini-1.5-pro", label: "Gemini 1.5 Pro" },
  ];

  const [modelOptions, setModelOptions] =
    useState<{ value: string; label: string }[]>(defaultGeminiModels);
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [isUpdating, setIsUpdating] = useState(false);

  // Access settings with type safety - these fields will exist after bindings regenerate
  const providerId = (settings as any)?.ramble_provider_id ?? "gemini";
  const model = (settings as any)?.ramble_model ?? "";
  const providers = settings?.post_process_providers ?? [];
  const apiKeys = settings?.post_process_api_keys ?? {};

  const useVisionModel = (settings as any)?.ramble_use_vision_model ?? false;
  const visionModel = (settings as any)?.ramble_vision_model ?? "";

  const selectedProvider = providers.find((p) => p.id === providerId);
  const apiKey = apiKeys[providerId] || "";

  // Sync prompt from settings
  useEffect(() => {
    const settingsPrompt = (settings as any)?.ramble_prompt;
    if (settingsPrompt) {
      setPrompt(settingsPrompt);
    }
  }, [(settings as any)?.ramble_prompt]);

  const handleProviderChange = async (newProviderId: string | null) => {
    if (!newProviderId) return;
    setIsUpdating(true);
    try {
      await commands.changeRambleProviderSetting(newProviderId);
      await refreshSettings();
      setModelOptions([]);
    } catch (error) {
      console.error("Failed to change ramble provider:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  const handleApiKeyChange = async (newApiKey: string) => {
    try {
      await commands.changePostProcessApiKeySetting(providerId, newApiKey);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change API key:", error);
    }
  };

  const handleModelChange = async (newModel: string) => {
    setIsUpdating(true);
    try {
      await commands.changeRambleModelSetting(newModel);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change ramble model:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  const handleUseVisionModelChange = async (enabled: boolean) => {
    try {
      await commands.changeRambleUseVisionModelSetting(enabled);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change use vision model setting:", error);
    }
  };

  const handleVisionModelChange = async (newModel: string) => {
    setIsUpdating(true);
    try {
      await commands.changeRambleVisionModelSetting(newModel);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change vision model:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  const handlePromptBlur = async () => {
    const currentPrompt = (settings as any)?.ramble_prompt ?? "";
    if (prompt.trim() !== currentPrompt.trim()) {
      try {
        await commands.changeRamblePromptSetting(prompt);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update ramble prompt:", error);
      }
    }
  };

  const handleResetPrompt = async () => {
    setIsUpdating(true);
    try {
      const result = await commands.resetRamblePromptToDefault();
      if (result.status === "ok") {
        setPrompt(result.data);
        await refreshSettings();
      }
    } catch (error) {
      console.error("Failed to reset ramble prompt:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  const handleRefreshModels = async () => {
    setIsFetchingModels(true);
    try {
      const result = await commands.fetchPostProcessModels(providerId);
      if (result.status === "ok") {
        setModelOptions(result.data.map((m) => ({ value: m, label: m })));
      }
    } catch (error) {
      console.error("Failed to fetch models:", error);
    } finally {
      setIsFetchingModels(false);
    }
  };

  const providerOptions = providers.map((p) => ({
    value: p.id,
    label: p.label,
  }));

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.ramble.title", "Ramble to Coherent")}>
        <div className="px-4 py-3 text-sm text-mid-gray">
          <p className="mb-2">
            <strong>
              {t("settings.ramble.howItWorks.title", "How it works:")}
            </strong>
          </p>
          <ul className="list-disc list-inside space-y-1">
            <li>
              {t(
                "settings.ramble.howItWorks.hold",
                "Hold the transcribe key → Raw transcription",
              )}
            </li>
            <li>
              {t(
                "settings.ramble.howItWorks.quickPress",
                "Quick tap → AI-powered text cleanup (refining)",
              )}
            </li>
          </ul>
        </div>
      </SettingsGroup>

      <SettingsGroup title={t("settings.ramble.llm.title", "AI Provider")}>
        <SettingContainer
          title={t("settings.ramble.provider.title", "Provider")}
          description={t(
            "settings.ramble.provider.description",
            "Select the AI provider for text cleanup.",
          )}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <ProviderSelect
            options={providerOptions}
            value={providerId}
            onChange={handleProviderChange}
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.ramble.apiKey.title", "API Key")}
          description={t(
            "settings.ramble.apiKey.description",
            "Your API key for the selected provider.",
          )}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <ApiKeyField
            value={apiKey}
            onBlur={handleApiKeyChange}
            placeholder={t(
              "settings.ramble.apiKey.placeholder",
              "Enter API key",
            )}
            disabled={false}
            className="min-w-[320px]"
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.ramble.model.title", "Model")}
          description={t(
            "settings.ramble.model.description",
            "Select or enter the model to use.",
          )}
          descriptionMode="tooltip"
          layout="stacked"
          grouped={true}
        >
          <div className="flex items-center gap-2">
            <ModelSelect
              value={model}
              options={modelOptions}
              disabled={false}
              isLoading={isFetchingModels}
              placeholder={
                modelOptions.length > 0
                  ? t(
                      "settings.ramble.model.placeholderWithOptions",
                      "Select a model",
                    )
                  : t(
                      "settings.ramble.model.placeholderNoOptions",
                      "Enter model name",
                    )
              }
              onSelect={handleModelChange}
              onCreate={handleModelChange}
              onBlur={() => {}}
              className="flex-1 min-w-[380px]"
            />
            <ResetButton
              onClick={handleRefreshModels}
              disabled={isFetchingModels}
              ariaLabel={t("settings.ramble.model.refresh", "Refresh models")}
            >
              <RefreshCcw
                className={`h-4 w-4 ${isFetchingModels ? "animate-spin" : ""}`}
              />
            </ResetButton>
          </div>
        </SettingContainer>

        <ToggleSwitch
          checked={useVisionModel}
          onChange={handleUseVisionModelChange}
          label={t(
            "settings.ramble.vision.useSpecialized.label",
            "Use different model for screenshots",
          )}
          description={t(
            "settings.ramble.vision.useSpecialized.description",
            "Route requests with screenshots to a more capable (or slower) model.",
          )}
          descriptionMode="tooltip"
          grouped={true}
        />

        {useVisionModel && (
          <SettingContainer
            title={t("settings.ramble.vision.model.title", "Screenshot Model")}
            description={t(
              "settings.ramble.vision.model.description",
              "Select the model to use when screenshots are attached.",
            )}
            descriptionMode="tooltip"
            layout="stacked"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <ModelSelect
                value={visionModel}
                options={modelOptions}
                disabled={false}
                isLoading={isFetchingModels}
                placeholder={
                  modelOptions.length > 0
                    ? t(
                        "settings.ramble.model.placeholderWithOptions",
                        "Select a model",
                      )
                    : t(
                        "settings.ramble.model.placeholderNoOptions",
                        "Enter model name",
                      )
                }
                onSelect={handleVisionModelChange}
                onCreate={handleVisionModelChange}
                onBlur={() => {}}
                className="flex-1 min-w-[380px]"
              />
              <ResetButton
                onClick={handleRefreshModels}
                disabled={isFetchingModels}
                ariaLabel={t("settings.ramble.model.refresh", "Refresh models")}
              >
                <RefreshCcw
                  className={`h-4 w-4 ${isFetchingModels ? "animate-spin" : ""}`}
                />
              </ResetButton>
            </div>
          </SettingContainer>
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.ramble.prompt.title", "Cleanup Prompt")}
      >
        <SettingContainer
          title={t(
            "settings.ramble.prompt.instructions.title",
            "Prompt Instructions",
          )}
          description={t(
            "settings.ramble.prompt.instructions.description",
            "Instructions for how the AI should clean up your speech. Use ${output} to reference the transcribed text.",
          )}
          descriptionMode="tooltip"
          layout="stacked"
          grouped={true}
        >
          <div className="space-y-2">
            <textarea
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              onBlur={handlePromptBlur}
              placeholder={t(
                "settings.ramble.prompt.placeholder",
                "Enter cleanup instructions...",
              )}
              className="w-full min-h-[200px] p-3 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary resize-y"
            />
            <button
              onClick={handleResetPrompt}
              disabled={isUpdating}
              className="flex items-center gap-2 px-3 py-1.5 text-sm bg-mid-gray/10 hover:bg-mid-gray/20 rounded-lg transition-colors disabled:opacity-50"
            >
              <RotateCcw className="h-4 w-4" />
              {t("settings.ramble.prompt.reset", "Restore Default Prompt")}
            </button>
          </div>
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
