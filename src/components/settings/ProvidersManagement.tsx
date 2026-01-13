import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, ChevronDown, ChevronRight } from "lucide-react";
import { commands, LLMProvider, LLMModel, DefaultModels } from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { ModelsDropdown } from "../ui/ModelsDropdown";
import { Dropdown } from "../ui/Dropdown";

// Available provider presets
const PROVIDER_PRESETS = [
  { id: "openai", name: "OpenAI", base_url: "https://api.openai.com/v1" },
  {
    id: "anthropic",
    name: "Anthropic",
    base_url: "https://api.anthropic.com/v1",
  },
  {
    id: "gemini",
    name: "Google Gemini",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
  },
  {
    id: "openrouter",
    name: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1",
  },
  { id: "custom", name: "Custom...", base_url: "" },
];

// ProviderRow: A single row in the dynamic provider list
interface ProviderRowProps {
  provider: LLMProvider;
  onUpdate: (apiKey: string) => void;
  onRemove: () => void;
  canRemove: boolean;
}

const ProviderRow: React.FC<ProviderRowProps> = ({
  provider,
  onUpdate,
  onRemove,
  canRemove,
}) => {
  const [apiKey, setApiKey] = useState(provider.api_key || "");

  const handleBlur = () => {
    if (apiKey !== (provider.api_key || "")) {
      onUpdate(apiKey);
    }
  };

  return (
    <div className="flex items-center gap-2 p-3 bg-mid-gray/5 rounded-lg border border-mid-gray/10">
      {/* Provider name (read-only) */}
      <div className="w-[140px] shrink-0">
        <span className="text-sm font-medium">{provider.name}</span>
        {provider.is_custom && (
          <span className="ml-1 text-[10px] text-mid-gray">(custom)</span>
        )}
      </div>

      {/* API Key input */}
      <input
        type="password"
        value={apiKey}
        onChange={(e) => setApiKey(e.target.value)}
        onBlur={handleBlur}
        placeholder="Enter API key..."
        className="flex-1 px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
      />

      {/* Configured indicator */}
      {provider.api_key && (
        <span className="px-2 py-1 text-xs text-green-600 bg-green-100 dark:bg-green-900/30 rounded">
          ✓
        </span>
      )}

      {/* Remove button */}
      <button
        onClick={onRemove}
        disabled={!canRemove}
        className="p-2 text-mid-gray hover:text-red-500 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        title={
          canRemove
            ? "Remove provider"
            : "Cannot remove - models are using this provider"
        }
      >
        <Trash2 className="h-4 w-4" />
      </button>
    </div>
  );
};

// ModelCheckbox: A checkbox for enabling/disabling a model
interface ModelCheckboxProps {
  model: LLMModel;
  providerName: string;
  onToggle: (enabled: boolean) => void;
}

const ModelCheckbox: React.FC<ModelCheckboxProps> = ({
  model,
  providerName,
  onToggle,
}) => {
  return (
    <label className="flex items-center gap-2 px-3 py-2 rounded hover:bg-mid-gray/5 cursor-pointer">
      <input
        type="checkbox"
        checked={model.enabled}
        onChange={(e) => onToggle(e.target.checked)}
        className="h-4 w-4 rounded border-mid-gray/30"
      />
      <span className="flex-1 text-sm">
        <span className="text-mid-gray text-xs">{providerName} /</span>{" "}
        {model.display_name}
      </span>
      {model.supports_vision && (
        <span className="text-[10px] text-mid-gray">vision</span>
      )}
    </label>
  );
};

export const ProvidersManagement: React.FC = () => {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<LLMProvider[]>([]);
  const [models, setModels] = useState<LLMModel[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModels | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(true);

  // Add provider state
  const [showAddProvider, setShowAddProvider] = useState(false);
  const [newProviderType, setNewProviderType] = useState("");
  const [customName, setCustomName] = useState("");
  const [customUrl, setCustomUrl] = useState("");

  // Load data
  const loadData = async () => {
    try {
      const [providersData, modelsData, defaultsData] = await Promise.all([
        commands.getLlmProviders(),
        commands.getLlmModels(),
        commands.getDefaultModels(),
      ]);
      setProviders(providersData);
      setModels(modelsData);
      setDefaultModels(defaultsData);
    } catch (error) {
      console.error("Failed to load provider data:", error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleUpdateApiKey = async (providerId: string, apiKey: string) => {
    try {
      await commands.updateProviderApiKey(providerId, apiKey);
      await loadData();
    } catch (error) {
      console.error("Failed to update API key:", error);
    }
  };

  const handleRemoveProvider = async (providerId: string) => {
    try {
      const result = await commands.deleteLlmProvider(providerId);
      if (result.status === "ok") {
        await loadData();
      }
    } catch (error) {
      console.error("Failed to remove provider:", error);
    }
  };

  const handleAddProvider = async () => {
    if (!newProviderType) return;

    try {
      let newProvider: LLMProvider;

      if (newProviderType === "custom") {
        if (!customName.trim() || !customUrl.trim()) return;
        newProvider = {
          id: crypto.randomUUID(),
          name: customName.trim(),
          base_url: customUrl.trim(),
          api_key: "",
          supports_vision: true,
          is_custom: true,
        };
      } else {
        const preset = PROVIDER_PRESETS.find((p) => p.id === newProviderType);
        if (!preset) return;
        newProvider = {
          id: newProviderType,
          name: preset.name,
          base_url: preset.base_url,
          api_key: "",
          supports_vision: true,
          is_custom: false,
        };
      }

      const result = await commands.saveLlmProvider(newProvider);
      if (result.status === "ok") {
        await loadData();
        // Reset add form
        setShowAddProvider(false);
        setNewProviderType("");
        setCustomName("");
        setCustomUrl("");
      }
    } catch (error) {
      console.error("Failed to add provider:", error);
    }
  };

  const handleToggleModel = async (modelId: string, enabled: boolean) => {
    const model = models.find((m) => m.id === modelId);
    if (!model) return;

    try {
      const result = await commands.saveLlmModel({ ...model, enabled });
      if (result.status === "ok") {
        await loadData();
      }
    } catch (error) {
      console.error("Failed to toggle model:", error);
    }
  };

  const handleSetDefaultModel = async (modelId: string | null) => {
    try {
      await commands.setDefaultModel("chat", modelId);
      await commands.setDefaultModel("coherent", modelId);
      await commands.setDefaultModel("voice", modelId);
      await commands.setDefaultModel("context_chat", modelId);
      await loadData();
    } catch (error) {
      console.error("Failed to set default model:", error);
    }
  };

  // Get list of provider IDs that can be added (not already in list)
  const availablePresets = PROVIDER_PRESETS.filter(
    (preset) =>
      preset.id === "custom" || !providers.some((p) => p.id === preset.id),
  );

  // Get enabled models for default dropdown
  const enabledModels = models.filter((m) => m.enabled);
  const defaultModelOptions = enabledModels.map((m) => {
    const provider = providers.find((p) => p.id === m.provider_id);
    return {
      value: m.id,
      label: `${provider?.name || m.provider_id} / ${m.display_name}`,
    };
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-40">
        <div className="animate-spin h-6 w-6 border-2 border-logo-primary border-t-transparent rounded-full" />
      </div>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Provider List - Dynamic Entries */}
      <SettingsGroup title="AI Providers">
        <div className="px-4 py-3 text-sm text-mid-gray">
          <p>
            Add your AI provider API keys below. You can add multiple providers.
          </p>
        </div>

        <div className="px-4 pb-4 space-y-2">
          {/* Provider rows */}
          {providers.map((provider) => (
            <ProviderRow
              key={provider.id}
              provider={provider}
              onUpdate={(apiKey) => handleUpdateApiKey(provider.id, apiKey)}
              onRemove={() => handleRemoveProvider(provider.id)}
              canRemove={provider.is_custom || false}
            />
          ))}

          {/* Add Provider Form */}
          {showAddProvider ? (
            <div className="p-3 border border-dashed border-mid-gray/30 rounded-lg space-y-3">
              <div className="flex items-center gap-2">
                <select
                  value={newProviderType}
                  onChange={(e) => setNewProviderType(e.target.value)}
                  className="flex-1 px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                >
                  <option value="">Select provider...</option>
                  {availablePresets.map((preset) => (
                    <option key={preset.id} value={preset.id}>
                      {preset.name}
                    </option>
                  ))}
                </select>
              </div>

              {newProviderType === "custom" && (
                <div className="space-y-2">
                  <input
                    type="text"
                    value={customName}
                    onChange={(e) => setCustomName(e.target.value)}
                    placeholder="Provider name..."
                    className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                  />
                  <input
                    type="text"
                    value={customUrl}
                    onChange={(e) => setCustomUrl(e.target.value)}
                    placeholder="API base URL (e.g., https://api.example.com/v1)"
                    className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                  />
                </div>
              )}

              <div className="flex justify-end gap-2">
                <button
                  onClick={() => {
                    setShowAddProvider(false);
                    setNewProviderType("");
                    setCustomName("");
                    setCustomUrl("");
                  }}
                  className="px-3 py-1.5 text-sm text-mid-gray hover:text-foreground"
                >
                  Cancel
                </button>
                <Button
                  onClick={handleAddProvider}
                  disabled={
                    !newProviderType ||
                    (newProviderType === "custom" &&
                      (!customName.trim() || !customUrl.trim()))
                  }
                  variant="primary"
                  size="sm"
                >
                  Add
                </Button>
              </div>
            </div>
          ) : (
            <button
              onClick={() => setShowAddProvider(true)}
              className="flex items-center gap-2 w-full p-3 border border-dashed border-mid-gray/30 rounded-lg text-sm text-mid-gray hover:text-foreground hover:border-mid-gray/50 transition-colors"
            >
              <Plus className="h-4 w-4" />
              Add Provider
            </button>
          )}
        </div>
      </SettingsGroup>
      {/* Model Selection */}
      <SettingsGroup title="Available Models">
        <div className="px-4 py-3 text-sm text-mid-gray">
          <p>
            Select which models should appear in model dropdowns throughout the
            app.
          </p>
        </div>

        <div className="px-4 pb-4">
          {models.length === 0 ? (
            <p className="text-sm text-mid-gray italic">
              No models available. Add a provider first.
            </p>
          ) : (
            <div className="grid gap-1">
              {models.map((model) => {
                const provider = providers.find(
                  (p) => p.id === model.provider_id,
                );
                return (
                  <ModelCheckbox
                    key={model.id}
                    model={model}
                    providerName={provider?.name || model.provider_id}
                    onToggle={(enabled) => handleToggleModel(model.id, enabled)}
                  />
                );
              })}
            </div>
          )}
        </div>
      </SettingsGroup>

      {/* Default Model Selection */}
      <SettingsGroup title="Default Model">
        <SettingContainer
          title="Default AI Model"
          description="This model will be used as the default across the app."
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <ModelsDropdown
            selectedValue={defaultModels?.chat || null}
            onSelect={handleSetDefaultModel}
            className="min-w-[300px]"
          />
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
