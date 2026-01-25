import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, X, Settings2, RefreshCcw } from "lucide-react";
import { commands, LLMProvider, LLMModel, DefaultModels } from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { ModelsDropdown } from "../ui/ModelsDropdown";
import { useSettings } from "../../hooks/useSettings";
import { ProviderAuth, OAuthStatusBadge } from "./ProviderAuth";
import { useOAuth } from "../../hooks/useOAuth";

// Known provider presets (models are fetched dynamically via API)
// API key providers and OAuth providers are separate entries
const PROVIDER_PRESETS: Record<
  string,
  {
    name: string;
    base_url: string;
    supports_oauth: boolean;
    auth_method: "api_key" | "oauth";
  }
> = {
  // API Key providers (original)
  openai: {
    name: "OpenAI",
    base_url: "https://api.openai.com/v1",
    supports_oauth: false,
    auth_method: "api_key",
  },
  anthropic: {
    name: "Anthropic",
    base_url: "https://api.anthropic.com/v1",
    supports_oauth: false,
    auth_method: "api_key",
  },
  gemini: {
    name: "Google Gemini",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    supports_oauth: false,
    auth_method: "api_key",
  },
  // OAuth providers (new - separate from API key providers)
  openai_oauth: {
    name: "OpenAI (OAuth)",
    base_url: "https://api.openai.com/v1",
    supports_oauth: true,
    auth_method: "oauth",
  },
  gemini_oauth: {
    name: "Google Gemini (OAuth)",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
    supports_oauth: true,
    auth_method: "oauth",
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
  existingProviders: LLMProvider[];
}

const ProviderDialog: React.FC<ProviderDialogProps> = ({
  isOpen,
  mode,
  provider,
  providerModels = [],
  onClose,
  onSave,
  onDelete,
  existingProviders,
}) => {
  const [providerType, setProviderType] = useState<"preset" | "custom">(
    "preset",
  );
  const [selectedPreset, setSelectedPreset] = useState<string>("");
  const [customName, setCustomName] = useState("");
  const [customUrl, setCustomUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [authMethod, setAuthMethod] = useState<"api_key" | "oauth">("api_key");
  const [selectedModels, setSelectedModels] = useState<Set<string>>(new Set());
  const [customModels, setCustomModels] = useState("");
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [fetchedModels, setFetchedModels] = useState<string[]>([]);
  const [fetchError, setFetchError] = useState<string | null>(null);

  // Get OAuth status for the current provider
  const currentProviderId =
    mode === "edit" ? (provider?.id ?? "") : selectedPreset || "";
  const { status: oauthStatus } = useOAuth(currentProviderId);

  // Check if current provider supports OAuth
  // Note: supports_oauth is optional in bindings until regenerated
  const currentSupportsOAuth =
    providerType === "preset" && selectedPreset
      ? (PROVIDER_PRESETS[selectedPreset]?.supports_oauth ?? false)
      : ((
          provider as LLMProvider & {
            supports_oauth?: boolean;
            auth_method?: string;
          }
        )?.supports_oauth ?? false);

  // Get the auth method from the preset (fixed per preset, not user-selectable)
  const presetAuthMethod =
    providerType === "preset" && selectedPreset
      ? PROVIDER_PRESETS[selectedPreset]?.auth_method
      : undefined;

  // Get current auth method from provider (defaults to api_key)
  const providerAuthMethod = (
    provider as LLMProvider & { auth_method?: string }
  )?.auth_method as "api_key" | "oauth" | undefined;

  // Initialize form when dialog opens
  useEffect(() => {
    if (isOpen) {
      setFetchError(null);
      setFetchedModels([]);
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
        // Set auth method from provider (default to api_key)
        setAuthMethod(providerAuthMethod || "api_key");
        // Set enabled models
        const enabledIds = new Set(
          providerModels.filter((m) => m.enabled).map((m) => m.model_id),
        );
        setSelectedModels(enabledIds);
        // For custom providers, load existing models into text field
        if (provider.is_custom && providerModels.length > 0) {
          const modelNames = providerModels.map((m) => m.model_id).join(", ");
          setCustomModels(modelNames);
        }
      } else {
        // Adding new provider
        // If no presets available, default to custom
        const hasAvailablePresets = Object.keys(PROVIDER_PRESETS).some(
          (id) => !existingProviders.some((p) => p.id === id && p.api_key),
        );
        setProviderType(hasAvailablePresets ? "preset" : "custom");
        setSelectedPreset("");
        setCustomName("");
        setCustomUrl("");
        setApiKey("");
        setAuthMethod("api_key");
        setSelectedModels(new Set());
        setCustomModels("");
      }
    }
  }, [isOpen, mode, provider, providerModels, providerAuthMethod]);

  // Update authMethod when preset changes (auth method is fixed per preset)
  useEffect(() => {
    if (
      providerType === "preset" &&
      selectedPreset &&
      PROVIDER_PRESETS[selectedPreset]
    ) {
      setAuthMethod(PROVIDER_PRESETS[selectedPreset].auth_method);
    }
  }, [providerType, selectedPreset]);

  const handleFetchModels = async () => {
    let providerIdToUse = provider?.id;

    setIsFetchingModels(true);
    setFetchError(null);
    try {
      // First, we must ensure the provider is saved with the current API key/URL
      // so the backend can use them to fetch models.
      // Use type assertion for extended provider fields
      type ExtendedLLMProvider = LLMProvider & {
        auth_method?: string;
        supports_oauth?: boolean;
      };
      let currentProvider: ExtendedLLMProvider;
      if (providerType === "preset") {
        const preset = PROVIDER_PRESETS[selectedPreset];
        currentProvider = {
          id: provider?.id || selectedPreset,
          name: preset.name,
          base_url: preset.base_url,
          api_key: preset.auth_method === "oauth" ? "" : apiKey,
          supports_vision: true,
          is_custom: false,
          auth_method: preset.auth_method,
          supports_oauth: preset.supports_oauth,
        };
      } else {
        currentProvider = {
          id: provider?.id || crypto.randomUUID(),
          name: customName,
          base_url: customUrl,
          api_key: apiKey,
          supports_vision: true,
          is_custom: true,
          auth_method: "api_key",
          supports_oauth: false,
        };
      }

      providerIdToUse = currentProvider.id;
      await commands.saveLlmProvider(currentProvider);

      const result = await commands.fetchPostProcessModels(providerIdToUse);
      if (result.status === "ok") {
        setFetchedModels(result.data);
        // If we're adding new models, auto-select them if none were selected or if they were already enabled
        const newSelectedModels = new Set(selectedModels);
        if (selectedModels.size === 0) {
          result.data.forEach((id) => newSelectedModels.add(id));
        }
        setSelectedModels(newSelectedModels);
      } else {
        setFetchError(result.error);
      }
    } catch (error) {
      setFetchError(String(error));
    } finally {
      setIsFetchingModels(false);
    }
  };

  // Available presets (filter out providers whose exact ID already exists with configuration)
  const availablePresets = Object.entries(PROVIDER_PRESETS).filter(
    ([id]) =>
      // Include if: (1) no existing provider with this exact ID has API key/OAuth, OR (2) we're editing this exact provider
      !existingProviders.some((p) => {
        if (p.id !== id) return false;
        const extProvider = p as LLMProvider & { auth_method?: string };
        // Provider is configured if it has API key OR uses OAuth
        return p.api_key || extProvider.auth_method === "oauth";
      }) ||
      (mode === "edit" && provider?.id === id),
  );

  // For custom providers editing, still need model management
  const toggleModel = (modelId: string) => {
    const newSet = new Set(selectedModels);
    if (newSet.has(modelId)) {
      newSet.delete(modelId);
    } else {
      newSet.add(modelId);
    }
    setSelectedModels(newSet);
  };

  const handleSave = () => {
    // Use type assertion for extended provider fields
    type ExtendedLLMProvider = LLMProvider & {
      auth_method?: string;
      supports_oauth?: boolean;
    };
    let providerData: ExtendedLLMProvider;
    let modelsToSave: { id: string; name: string; vision?: boolean }[] = [];
    let enabledIds = new Set(selectedModels);

    const allKnownModelIdsFromCheckboxes = Array.from(
      new Set([...fetchedModels, ...providerModels.map((m) => m.model_id)]),
    );

    if (providerType === "preset" && selectedPreset) {
      const preset = PROVIDER_PRESETS[selectedPreset];
      // Auth method is fixed per preset (OAuth providers use OAuth, API key providers use API key)
      const effectiveAuthMethod = preset.auth_method;
      providerData = {
        id: selectedPreset,
        name: preset.name,
        base_url: preset.base_url,
        api_key: effectiveAuthMethod === "oauth" ? "" : apiKey,
        supports_vision: true,
        is_custom: false,
        auth_method: effectiveAuthMethod,
        supports_oauth: preset.supports_oauth,
      };
      modelsToSave = allKnownModelIdsFromCheckboxes.map((id) => {
        const existing = providerModels.find((m) => m.model_id === id);
        return {
          id: id,
          name: id,
          vision: existing?.supports_vision || false,
        };
      });
    } else if (providerType === "custom" && customName && customUrl) {
      providerData = {
        id: provider?.id || crypto.randomUUID(),
        name: customName,
        base_url: customUrl,
        api_key: apiKey,
        supports_vision: true,
        is_custom: true,
        auth_method: "api_key", // Custom providers always use API key
        supports_oauth: false,
      };

      // Merge models from text field and fetched models
      const textModelIds = customModels
        .split(",")
        .map((m) => m.trim())
        .filter(Boolean);
      const allModelIds = Array.from(
        new Set([...textModelIds, ...allKnownModelIdsFromCheckboxes]),
      );

      modelsToSave = allModelIds.map((id) => {
        const existing = providerModels.find((m) => m.model_id === id);
        return {
          id: id,
          name: id,
          vision: existing?.supports_vision || false,
        };
      });

      // Auto-enable models newly added in the text field
      textModelIds.forEach((id) => {
        if (!providerModels.some((m) => m.model_id === id)) {
          enabledIds.add(id);
        }
      });
    } else {
      return;
    }

    onSave(providerData, modelsToSave, enabledIds);
    onClose();
  };

  // Determine if we can save based on auth method
  // For OAuth: must be authenticated (oauthStatus?.authenticated)
  // For API key: must have a valid API key
  const hasValidAuth =
    authMethod === "oauth"
      ? oauthStatus?.authenticated === true
      : apiKey.trim().length > 0;

  const canSave =
    hasValidAuth &&
    ((providerType === "preset" && selectedPreset) ||
      (providerType === "custom" && customName.trim() && customUrl.trim()));

  if (!isOpen) return null;

  const allModelIds = Array.from(
    new Set([...fetchedModels, ...providerModels.map((m) => m.model_id)]),
  ).sort();

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="bg-background border border-mid-gray/20 rounded-xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-hidden flex flex-col">
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
                      setFetchedModels([]);
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

              {/* Authentication - use ProviderAuth for OAuth-capable providers */}
              <ProviderAuth
                providerId={currentProviderId}
                supportsOAuth={currentSupportsOAuth}
                authMethod={authMethod}
                apiKey={apiKey}
                onAuthMethodChange={setAuthMethod}
                onApiKeyChange={setApiKey}
                fixedAuthMethod={providerType === "preset"}
              />

              {/* Models List */}
              {(selectedPreset || mode === "edit") && (
                <div className="space-y-2 border-t border-mid-gray/10 pt-4">
                  <div className="flex items-center justify-between">
                    <label className="text-sm font-medium">Models</label>
                    <div className="flex items-center gap-2">
                      {allModelIds.length > 0 && (
                        <>
                          <button
                            onClick={() =>
                              setSelectedModels(new Set(allModelIds))
                            }
                            className="text-xs text-logo-primary hover:underline"
                          >
                            Check All
                          </button>
                          <span className="text-mid-gray/30 text-xs">|</span>
                          <button
                            onClick={() => setSelectedModels(new Set())}
                            className="text-xs text-logo-primary hover:underline"
                          >
                            Uncheck All
                          </button>
                          <span className="text-mid-gray/30 text-xs">|</span>
                        </>
                      )}
                      <button
                        onClick={handleFetchModels}
                        disabled={isFetchingModels || !hasValidAuth}
                        className="p-1 text-mid-gray hover:text-logo-primary transition-colors disabled:opacity-50"
                        title="Refresh models"
                      >
                        <RefreshCcw
                          className={`h-4 w-4 ${isFetchingModels ? "animate-spin" : ""}`}
                        />
                      </button>
                    </div>
                  </div>

                  {fetchError && (
                    <p className="text-xs text-red-500">{fetchError}</p>
                  )}

                  {allModelIds.length > 0 ? (
                    <div className="grid grid-cols-1 gap-1 max-h-48 overflow-y-auto pr-2 border border-mid-gray/10 rounded-lg p-2">
                      {allModelIds.map((modelId) => (
                        <label
                          key={modelId}
                          className="flex items-center gap-2 p-1.5 hover:bg-mid-gray/5 rounded cursor-pointer text-sm"
                        >
                          <input
                            type="checkbox"
                            checked={selectedModels.has(modelId)}
                            onChange={() => toggleModel(modelId)}
                            className="rounded border-mid-gray/30 text-logo-primary focus:ring-logo-primary"
                          />
                          <span className="truncate">{modelId}</span>
                        </label>
                      ))}
                    </div>
                  ) : (
                    <div className="p-3 bg-mid-gray/5 border border-mid-gray/20 rounded-lg">
                      <p className="text-sm text-mid-gray">
                        {currentSupportsOAuth
                          ? "Sign in or enter your API key, then click the "
                          : "Enter your API key and click the "}
                        <strong>Refresh</strong> button to fetch available
                        models.
                      </p>
                    </div>
                  )}
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
                <div className="flex items-center justify-between">
                  <label className="text-sm font-medium">
                    Models (comma-separated)
                  </label>
                  <button
                    onClick={handleFetchModels}
                    disabled={
                      isFetchingModels || !apiKey.trim() || !customUrl.trim()
                    }
                    className="p-1 text-mid-gray hover:text-logo-primary transition-colors disabled:opacity-50"
                    title="Refresh models"
                  >
                    <RefreshCcw
                      className={`h-4 w-4 ${isFetchingModels ? "animate-spin" : ""}`}
                    />
                  </button>
                </div>
                <input
                  type="text"
                  value={customModels}
                  onChange={(e) => setCustomModels(e.target.value)}
                  placeholder="model-1, model-2, model-3"
                  className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                />
              </div>

              {/* Models List for Custom Provider (if fetched) */}
              {allModelIds.length > 0 && (
                <div className="space-y-2 border-t border-mid-gray/10 pt-4">
                  <div className="flex items-center justify-between">
                    <label className="text-sm font-medium">
                      Enabled Models
                    </label>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => setSelectedModels(new Set(allModelIds))}
                        className="text-xs text-logo-primary hover:underline"
                      >
                        Check All
                      </button>
                      <span className="text-mid-gray/30 text-xs">|</span>
                      <button
                        onClick={() => setSelectedModels(new Set())}
                        className="text-xs text-logo-primary hover:underline"
                      >
                        Uncheck All
                      </button>
                    </div>
                  </div>

                  <div className="grid grid-cols-1 gap-1 max-h-48 overflow-y-auto pr-2 border border-mid-gray/10 rounded-lg p-2">
                    {allModelIds.map((modelId) => (
                      <label
                        key={modelId}
                        className="flex items-center gap-2 p-1.5 hover:bg-mid-gray/5 rounded cursor-pointer text-sm"
                      >
                        <input
                          type="checkbox"
                          checked={selectedModels.has(modelId)}
                          onChange={() => toggleModel(modelId)}
                          className="rounded border-mid-gray/30 text-logo-primary focus:ring-logo-primary"
                        />
                        <span className="truncate">{modelId}</span>
                      </label>
                    ))}
                  </div>
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-between px-6 py-4 border-t border-mid-gray/10 bg-mid-gray/5">
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
  const {
    settings,
    isLoading: settingsLoading,
    refreshSettings,
  } = useSettings();

  const providers = settings?.llm_providers || [];
  const models = settings?.llm_models || [];

  const [isLoading, setIsLoading] = useState(false);

  // Dialog state
  const [dialogMode, setDialogMode] = useState<"add" | "edit">("add");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<LLMProvider | null>(
    null,
  );

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

      await refreshSettings();
    } catch (error) {
      console.error("Failed to save provider:", error);
    }
  };

  const handleDeleteProvider = async () => {
    if (!editingProvider) return;
    try {
      await commands.deleteLlmProvider(editingProvider.id);
      setDialogOpen(false);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to delete provider:", error);
    }
  };

  const handleSetDefaultModel = async (modelId: string | null) => {
    try {
      await commands.setDefaultModel("chat", modelId);
      await commands.setDefaultModel("coherent", modelId);
      await commands.setDefaultModel("voice", modelId);
      await commands.setDefaultModel("context_chat", modelId);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to set default model:", error);
    }
  };

  // Get models for a provider
  const getProviderModels = (providerId: string) =>
    models.filter((m) => m.provider_id === providerId);

  // Filter for enabled models and ensure provider has API key OR OAuth
  const configuredProviderIds = new Set(
    providers
      .filter((p) => {
        const extProvider = p as LLMProvider & { auth_method?: string };
        // Provider is configured if it has an API key OR uses OAuth
        return (
          (p.api_key && p.api_key.trim() !== "") ||
          extProvider.auth_method === "oauth"
        );
      })
      .map((p) => p.id),
  );
  const anyModelsEnabled = models.some(
    (m) =>
      m.enabled === true &&
      m.provider_id &&
      configuredProviderIds.has(m.provider_id),
  );

  if (settingsLoading) {
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
          {/* Provider rows - show providers with API keys OR OAuth auth */}
          {providers
            .filter((p) => {
              const extProvider = p as LLMProvider & {
                auth_method?: string;
                supports_oauth?: boolean;
              };
              // Show if has API key OR uses OAuth
              return p.api_key || extProvider.auth_method === "oauth";
            })
            .map((provider) => {
              const providerModels = getProviderModels(provider.id);
              const enabledCount = providerModels.filter(
                (m) => m.enabled,
              ).length;
              const extProvider = provider as LLMProvider & {
                auth_method?: string;
                supports_oauth?: boolean;
              };
              const usesOAuth = extProvider.auth_method === "oauth";

              return (
                <button
                  key={provider.id}
                  onClick={() => openEditDialog(provider)}
                  className="flex items-center gap-3 w-full p-3 bg-mid-gray/5 hover:bg-mid-gray/10 rounded-lg border border-mid-gray/10 transition-colors text-left"
                >
                  <span className="font-medium text-sm flex-1">
                    {provider.name}
                  </span>

                  {usesOAuth ? (
                    <OAuthStatusBadge
                      providerId={provider.id}
                      supportsOAuth={extProvider.supports_oauth ?? false}
                      authMethod="oauth"
                    />
                  ) : provider.api_key ? (
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
            selectedValue={settings?.default_chat_model_id || null}
            onSelect={handleSetDefaultModel}
            disabled={!anyModelsEnabled}
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
        existingProviders={providers}
      />
    </div>
  );
};
