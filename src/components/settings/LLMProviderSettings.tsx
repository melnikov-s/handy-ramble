import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCcw } from "lucide-react";
import { commands } from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { ResetButton } from "../ui/ResetButton";
import { ProviderSelect } from "./PostProcessingSettingsApi/ProviderSelect";
import { ApiKeyField } from "./PostProcessingSettingsApi/ApiKeyField";
import { useSettings } from "../../hooks/useSettings";

// Type assertion for new commands (until bindings regenerate)
interface LLMProviderCommands {
  changeLlmProviderSetting: (providerId: string) => Promise<any>;
}
const extendedCommands = commands as unknown as LLMProviderCommands &
  typeof commands;

export const LLMProviderSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();

  const [isUpdating, setIsUpdating] = useState(false);
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const [fetchSuccess, setFetchSuccess] = useState(false);

  // Get current provider state
  const providerId = (settings as any)?.llm_provider_id ?? "gemini";
  const providers = settings?.post_process_providers ?? [];
  const apiKeys = settings?.post_process_api_keys ?? {};
  const selectedProvider = providers.find((p) => p.id === providerId);
  const apiKey = apiKeys[providerId] || "";

  const providerOptions = providers.map((p) => ({
    value: p.id,
    label: p.label,
  }));

  const handleProviderChange = async (newProviderId: string | null) => {
    if (!newProviderId) return;
    setIsUpdating(true);
    setFetchError(null);
    setFetchSuccess(false);
    try {
      await extendedCommands.changeLlmProviderSetting(newProviderId);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change LLM provider:", error);
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

  const handleTestConnection = async () => {
    setIsFetchingModels(true);
    setFetchError(null);
    setFetchSuccess(false);
    try {
      const result = await commands.fetchPostProcessModels(providerId);
      if (result.status === "ok" && result.data.length > 0) {
        setFetchSuccess(true);
      } else if (result.status === "error") {
        setFetchError(result.error);
      }
    } catch (error) {
      setFetchError(String(error));
    } finally {
      setIsFetchingModels(false);
    }
  };

  return (
    <SettingsGroup title={t("settings.llmProvider.title", "AI Provider")}>
      <div className="px-4 py-2 text-sm text-mid-gray">
        <p>
          {t(
            "settings.llmProvider.description",
            "Configure the AI provider for Refiner, Post-Processing, and Voice Commands.",
          )}
        </p>
      </div>

      <SettingContainer
        title={t("settings.llmProvider.provider.title", "Provider")}
        description={t(
          "settings.llmProvider.provider.description",
          "Select the AI provider to use for all AI features.",
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
        title={t("settings.llmProvider.apiKey.title", "API Key")}
        description={t(
          "settings.llmProvider.apiKey.description",
          "Your API key for the selected provider.",
        )}
        descriptionMode="tooltip"
        layout="horizontal"
        grouped={true}
      >
        <div className="flex items-center gap-2">
          <ApiKeyField
            value={apiKey}
            onBlur={handleApiKeyChange}
            placeholder={t(
              "settings.llmProvider.apiKey.placeholder",
              `Enter ${selectedProvider?.label || "API"} key`,
            )}
            disabled={false}
            className="min-w-[280px]"
          />
          <ResetButton
            onClick={handleTestConnection}
            disabled={isFetchingModels || !apiKey}
            ariaLabel={t("settings.llmProvider.test", "Test connection")}
          >
            <RefreshCcw
              className={`h-4 w-4 ${isFetchingModels ? "animate-spin" : ""}`}
            />
          </ResetButton>
        </div>
      </SettingContainer>

      {fetchSuccess && (
        <div className="px-4 py-2 text-sm text-green-600 bg-green-50 dark:bg-green-900/20 dark:text-green-400 rounded-lg mx-4 mb-3">
          ✓ {t("settings.llmProvider.success", "Connection successful!")}
        </div>
      )}

      {fetchError && (
        <div className="px-4 py-2 text-sm text-red-600 bg-red-50 dark:bg-red-900/20 dark:text-red-400 rounded-lg mx-4 mb-3">
          {fetchError}
        </div>
      )}
    </SettingsGroup>
  );
};
