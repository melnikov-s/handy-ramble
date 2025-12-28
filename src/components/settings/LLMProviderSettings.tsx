import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, X, Settings2 } from "lucide-react";
import { commands, LLMProvider, LLMModel, DefaultModels } from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { ModelsDropdown } from "../ui/ModelsDropdown";

// Known provider presets with their models
const PROVIDER_PRESETS: Record<
  string,
  {
    name: string;
    base_url: string;
    models: { id: string; name: string; vision?: boolean }[];
  }
> = {
  openai: {
    name: "OpenAI",
    base_url: "https://api.openai.com/v1",
    models: [
      { id: "gpt-4o", name: "gpt-4o", vision: true },
      { id: "gpt-4o-mini", name: "gpt-4o-mini", vision: true },
      { id: "gpt-4-turbo", name: "gpt-4-turbo", vision: true },
      { id: "o1", name: "o1", vision: true },
      { id: "o1-mini", name: "o1-mini" },
      { id: "o3-mini", name: "o3-mini" },
    ],
  },
  anthropic: {
    name: "Anthropic",
    base_url: "https://api.anthropic.com/v1",
    models: [
      {
        id: "claude-sonnet-4-20250514",
        name: "claude-sonnet-4-20250514",
        vision: true,
      },
      {
        id: "claude-3-5-sonnet-latest",
        name: "claude-3-5-sonnet-latest",
        vision: true,
      },
      {
        id: "claude-3-5-haiku-latest",
        name: "claude-3-5-haiku-latest",
        vision: true,
      },
      {
        id: "claude-3-opus-latest",
        name: "claude-3-opus-latest",
        vision: true,
      },
    ],
  },
  gemini: {
    name: "Google Gemini",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    models: [
      // Gemini 3 (latest)
      {
        id: "gemini-3-flash-preview",
        name: "gemini-3-flash-preview",
        vision: true,
      },
      // Gemini 2.5 series
      { id: "gemini-2.5-pro", name: "gemini-2.5-pro", vision: true },
      { id: "gemini-2.5-pro-lite", name: "gemini-2.5-pro-lite", vision: true },
      { id: "gemini-2.5-flash", name: "gemini-2.5-flash", vision: true },
      {
        id: "gemini-2.5-flash-lite",
        name: "gemini-2.5-flash-lite",
        vision: true,
      },
      // Gemini 2.0 series
      { id: "gemini-2.0-flash", name: "gemini-2.0-flash", vision: true },
      {
        id: "gemini-2.0-flash-lite",
        name: "gemini-2.0-flash-lite",
        vision: true,
      },
      // Gemini 1.5 series
      { id: "gemini-1.5-flash", name: "gemini-1.5-flash", vision: true },
      { id: "gemini-1.5-pro", name: "gemini-1.5-pro", vision: true },
    ],
  },
  openrouter: {
    name: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1",
    models: [
      {
        id: "anthropic/claude-sonnet-4",
        name: "anthropic/claude-sonnet-4",
        vision: true,
      },
      { id: "openai/gpt-4o", name: "openai/gpt-4o", vision: true },
      {
        id: "google/gemini-2.0-flash-001",
        name: "google/gemini-2.0-flash-001",
        vision: true,
      },
      { id: "deepseek/deepseek-r1", name: "deepseek/deepseek-r1" },
      {
        id: "meta-llama/llama-3.3-70b-instruct",
        name: "meta-llama/llama-3.3-70b-instruct",
      },
    ],
  },
};

// Provider Dialog (Add or Edit)
interface ProviderDialogProps {
  isOpen: boolean;
  mode: "add" | "edit";
  provider?: LLMProvider;
  providerModels?: LLMModel[];
  onClose: () => void;
  onSave: (
    provider: LLMProvider,
    selectedModels: { id: string; name: string; vision?: boolean }[],
    enabledModelIds: Set<string>,
  ) => void;
  onDelete?: () => void;
  existingProviderIds: string[];
}

const ProviderDialog: React.FC<ProviderDialogProps> = ({
  isOpen,
  mode,
  provider,
  providerModels = [],
  onClose,
  onSave,
  onDelete,
  existingProviderIds,
}) => {
  const [providerType, setProviderType] = useState<"preset" | "custom">(
    "preset",
  );
  const [selectedPreset, setSelectedPreset] = useState<string>("");
  const [customName, setCustomName] = useState("");
  const [customUrl, setCustomUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [selectedModels, setSelectedModels] = useState<Set<string>>(new Set());
  const [customModels, setCustomModels] = useState("");

  // Initialize form when dialog opens
  useEffect(() => {
    if (isOpen) {
      if (mode === "edit" && provider) {
        // Editing existing provider
        if (provider.is_custom) {
          setProviderType("custom");
          setCustomName(provider.name);
          setCustomUrl(provider.base_url);
        } else {
          setProviderType("preset");
          setSelectedPreset(provider.id);
        }
        setApiKey(provider.api_key || "");
        // Set enabled models
        const enabledIds = new Set(
          providerModels.filter((m) => m.enabled).map((m) => m.model_id),
        );
        setSelectedModels(enabledIds);
      } else {
        // Adding new provider
        // If no presets available, default to custom
        const hasAvailablePresets = Object.keys(PROVIDER_PRESETS).some(
          (id) => !existingProviderIds.includes(id),
        );
        setProviderType(hasAvailablePresets ? "preset" : "custom");
        setSelectedPreset("");
        setCustomName("");
        setCustomUrl("");
        setApiKey("");
        setSelectedModels(new Set());
        setCustomModels("");
      }
    }
  }, [isOpen, mode, provider, providerModels]);

  // Available presets (filter out already added, unless editing that one)
  const availablePresets = Object.entries(PROVIDER_PRESETS).filter(
    ([id]) =>
      !existingProviderIds.includes(id) ||
      (mode === "edit" && provider?.id === id),
  );

  // Get models for current selection
  const getModels = (): { id: string; name: string; vision?: boolean }[] => {
    if (providerType === "preset" && selectedPreset) {
      return PROVIDER_PRESETS[selectedPreset]?.models || [];
    }
    return [];
  };

  const currentModels = getModels();

  const toggleModel = (modelId: string) => {
    const newSet = new Set(selectedModels);
    if (newSet.has(modelId)) {
      newSet.delete(modelId);
    } else {
      newSet.add(modelId);
    }
    setSelectedModels(newSet);
  };

  const selectAllModels = () => {
    setSelectedModels(new Set(currentModels.map((m) => m.id)));
  };

  const deselectAllModels = () => {
    setSelectedModels(new Set());
  };

  const handleSave = () => {
    let providerData: LLMProvider;
    let models: { id: string; name: string; vision?: boolean }[] = [];

    if (providerType === "preset" && selectedPreset) {
      const preset = PROVIDER_PRESETS[selectedPreset];
      providerData = {
        id: selectedPreset,
        name: preset.name,
        base_url: preset.base_url,
        api_key: apiKey,
        supports_vision: true,
        is_custom: false,
      };
      models = currentModels;
    } else if (providerType === "custom" && customName && customUrl) {
      providerData = {
        id: provider?.id || crypto.randomUUID(),
        name: customName,
        base_url: customUrl,
        api_key: apiKey,
        supports_vision: true,
        is_custom: true,
      };
      // Parse custom models
      if (customModels.trim()) {
        models = customModels.split(",").map((m) => ({
          id: m.trim(),
          name: m.trim(),
          vision: false,
        }));
      }
    } else {
      return;
    }

    onSave(providerData, models, selectedModels);
    onClose();
  };

  // API key is REQUIRED to save
  const canSave =
    apiKey.trim() &&
    ((providerType === "preset" && selectedPreset) ||
      (providerType === "custom" && customName.trim() && customUrl.trim()));

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-background border border-mid-gray/20 rounded-xl shadow-xl w-full max-w-lg max-h-[85vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-mid-gray/10">
          <h2 className="text-lg font-semibold">
            {mode === "add"
              ? "Add AI Provider"
              : `Edit ${provider?.name || "Provider"}`}
          </h2>
          <button
            onClick={onClose}
            className="p-1 text-mid-gray hover:text-foreground transition-colors"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
          {/* Provider Type Toggle (only for add mode AND when presets available) */}
          {mode === "add" && availablePresets.length > 0 && (
            <div className="flex gap-2">
              <button
                onClick={() => setProviderType("preset")}
                className={`flex-1 py-2 px-4 text-sm rounded-lg border transition-colors ${
                  providerType === "preset"
                    ? "bg-logo-primary text-white border-logo-primary"
                    : "bg-mid-gray/5 border-mid-gray/20 hover:border-mid-gray/40"
                }`}
              >
                Known Provider
              </button>
              <button
                onClick={() => setProviderType("custom")}
                className={`flex-1 py-2 px-4 text-sm rounded-lg border transition-colors ${
                  providerType === "custom"
                    ? "bg-logo-primary text-white border-logo-primary"
                    : "bg-mid-gray/5 border-mid-gray/20 hover:border-mid-gray/40"
                }`}
              >
                Custom Provider
              </button>
            </div>
          )}

          {providerType === "preset" ? (
            <>
              {/* Preset Selection */}
              {mode === "add" && (
                <div className="space-y-2">
                  <label className="text-sm font-medium">Provider</label>
                  <select
                    value={selectedPreset}
                    onChange={(e) => {
                      setSelectedPreset(e.target.value);
                      setSelectedModels(new Set());
                    }}
                    className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                  >
                    <option value="">Select a provider...</option>
                    {availablePresets.map(([id, preset]) => (
                      <option key={id} value={id}>
                        {preset.name}
                      </option>
                    ))}
                  </select>
                </div>
              )}

              {/* API Key */}
              <div className="space-y-2">
                <label className="text-sm font-medium">API Key</label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Enter API key..."
                  className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>

              {/* Model Selection */}
              {(selectedPreset || mode === "edit") &&
                currentModels.length > 0 && (
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <label className="text-sm font-medium">Models</label>
                      <div className="flex gap-2 text-xs">
                        <button
                          onClick={selectAllModels}
                          className="text-logo-primary hover:underline"
                        >
                          Select All
                        </button>
                        <span className="text-mid-gray">|</span>
                        <button
                          onClick={deselectAllModels}
                          className="text-mid-gray hover:text-foreground"
                        >
                          Clear
                        </button>
                      </div>
                    </div>
                    <div className="border border-mid-gray/20 rounded-lg max-h-48 overflow-y-auto">
                      {currentModels.map((model) => (
                        <label
                          key={model.id}
                          className="flex items-center gap-2 px-3 py-2 hover:bg-mid-gray/5 cursor-pointer"
                        >
                          <input
                            type="checkbox"
                            checked={selectedModels.has(model.id)}
                            onChange={() => toggleModel(model.id)}
                            className="h-4 w-4 rounded"
                          />
                          <span className="text-sm flex-1">{model.name}</span>
                          {model.vision && (
                            <span className="text-[10px] text-mid-gray">
                              vision
                            </span>
                          )}
                        </label>
                      ))}
                    </div>
                  </div>
                )}
            </>
          ) : (
            <>
              {/* Custom Provider Fields */}
              <div className="space-y-2">
                <label className="text-sm font-medium">Provider Name</label>
                <input
                  type="text"
                  value={customName}
                  onChange={(e) => setCustomName(e.target.value)}
                  placeholder="My Custom Provider"
                  className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">API Base URL</label>
                <input
                  type="text"
                  value={customUrl}
                  onChange={(e) => setCustomUrl(e.target.value)}
                  placeholder="https://api.example.com/v1"
                  className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">API Key</label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Enter API key..."
                  className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">
                  Models (comma-separated)
                </label>
                <input
                  type="text"
                  value={customModels}
                  onChange={(e) => setCustomModels(e.target.value)}
                  placeholder="model-1, model-2, model-3"
                  className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-between px-6 py-4 border-t border-mid-gray/10">
          <div>
            {mode === "edit" && onDelete && (
              <button
                onClick={onDelete}
                className="px-3 py-2 text-sm text-red-500 hover:text-red-600 transition-colors"
              >
                Delete Provider
              </button>
            )}
          </div>
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 text-sm text-mid-gray hover:text-foreground transition-colors"
            >
              Cancel
            </button>
            <Button onClick={handleSave} disabled={!canSave} variant="primary">
              {mode === "add" ? "Add Provider" : "Save Changes"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};

// Main Component
export const LLMProviderSettings: React.FC = () => {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<LLMProvider[]>([]);
  const [models, setModels] = useState<LLMModel[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModels | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(true);

  // Dialog state
  const [dialogMode, setDialogMode] = useState<"add" | "edit">("add");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<LLMProvider | null>(
    null,
  );

  const loadData = async () => {
    try {
      const [p, m, d] = await Promise.all([
        commands.getLlmProviders(),
        commands.getLlmModels(),
        commands.getDefaultModels(),
      ]);
      setProviders(p);
      setModels(m);
      setDefaultModels(d);
    } catch (error) {
      console.error("Failed to load data:", error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const openAddDialog = () => {
    setDialogMode("add");
    setEditingProvider(null);
    setDialogOpen(true);
  };

  const openEditDialog = (provider: LLMProvider) => {
    setDialogMode("edit");
    setEditingProvider(provider);
    setDialogOpen(true);
  };

  const handleSaveProvider = async (
    provider: LLMProvider,
    allModels: { id: string; name: string; vision?: boolean }[],
    enabledModelIds: Set<string>,
  ) => {
    try {
      // Save provider
      await commands.saveLlmProvider(provider);

      // Save/update models
      for (const m of allModels) {
        const modelId = `${provider.id}-${m.id.replace(/\//g, "-")}`;
        const newModel: LLMModel = {
          id: modelId,
          provider_id: provider.id,
          model_id: m.id,
          display_name: m.name,
          supports_vision: m.vision || false,
          enabled: enabledModelIds.has(m.id),
        };
        await commands.saveLlmModel(newModel);
      }

      await loadData();
    } catch (error) {
      console.error("Failed to save provider:", error);
    }
  };

  const handleDeleteProvider = async () => {
    if (!editingProvider) return;
    try {
      await commands.deleteLlmProvider(editingProvider.id);
      setDialogOpen(false);
      await loadData();
    } catch (error) {
      console.error("Failed to delete provider:", error);
    }
  };

  const handleSetDefaultModel = async (modelId: string | null) => {
    try {
      await commands.setDefaultModel("chat", modelId);
      await commands.setDefaultModel("coherent", modelId);
      await commands.setDefaultModel("voice", modelId);
      await loadData();
    } catch (error) {
      console.error("Failed to set default model:", error);
    }
  };

  // Get models for a provider
  const getProviderModels = (providerId: string) =>
    models.filter((m) => m.provider_id === providerId);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-40">
        <div className="animate-spin h-6 w-6 border-2 border-logo-primary border-t-transparent rounded-full" />
      </div>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Provider List */}
      <SettingsGroup title={t("settings.llmProvider.title", "AI Providers")}>
        <div className="px-4 py-2 text-sm text-mid-gray">
          <p>Add and configure your AI providers.</p>
        </div>

        <div className="px-4 pb-4 space-y-2">
          {/* Provider rows - only show providers with API keys */}
          {providers
            .filter((p) => p.api_key)
            .map((provider) => {
              const providerModels = getProviderModels(provider.id);
              const enabledCount = providerModels.filter(
                (m) => m.enabled,
              ).length;

              return (
                <button
                  key={provider.id}
                  onClick={() => openEditDialog(provider)}
                  className="flex items-center gap-3 w-full p-3 bg-mid-gray/5 hover:bg-mid-gray/10 rounded-lg border border-mid-gray/10 transition-colors text-left"
                >
                  <span className="font-medium text-sm flex-1">
                    {provider.name}
                  </span>

                  {provider.api_key ? (
                    <span className="text-xs text-green-600 bg-green-100 dark:bg-green-900/30 px-2 py-0.5 rounded">
                      Configured
                    </span>
                  ) : (
                    <span className="text-xs text-orange-600 bg-orange-100 dark:bg-orange-900/30 px-2 py-0.5 rounded">
                      No API Key
                    </span>
                  )}

                  <span className="text-xs text-mid-gray">
                    {enabledCount} model{enabledCount !== 1 ? "s" : ""}
                  </span>

                  <Settings2 className="h-4 w-4 text-mid-gray" />
                </button>
              );
            })}

          {/* Empty state */}
          {providers.length === 0 && (
            <div className="text-center py-8 text-mid-gray">
              <p className="text-sm">No providers configured yet.</p>
              <p className="text-xs mt-1">
                Click "Add Provider" to get started.
              </p>
            </div>
          )}

          {/* Add Provider Button */}
          <button
            onClick={openAddDialog}
            className="flex items-center gap-2 w-full p-3 border border-dashed border-mid-gray/30 rounded-lg text-sm text-mid-gray hover:text-foreground hover:border-mid-gray/50 transition-colors"
          >
            <Plus className="h-4 w-4" />
            Add Provider
          </button>
        </div>
      </SettingsGroup>

      {/* Default Model */}
      <SettingsGroup title={t("settings.llmProvider.default", "Default Model")}>
        <SettingContainer
          title={t("settings.providers.defaultModel.title", "Default Model")}
          description={t(
            "settings.providers.defaultModel.description",
            "Set the default model for all features (chat, coherent mode, voice commands).",
          )}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <ModelsDropdown
            selectedValue={defaultModels?.chat || null}
            onSelect={handleSetDefaultModel}
            disabled={models.filter((m) => m.enabled).length === 0}
            className="min-w-[280px]"
          />
        </SettingContainer>
      </SettingsGroup>

      {/* Provider Dialog */}
      <ProviderDialog
        isOpen={dialogOpen}
        mode={dialogMode}
        provider={editingProvider || undefined}
        providerModels={
          editingProvider ? getProviderModels(editingProvider.id) : []
        }
        onClose={() => setDialogOpen(false)}
        onSave={handleSaveProvider}
        onDelete={handleDeleteProvider}
        existingProviderIds={providers.map((p) => p.id)}
      />
    </div>
  );
};
