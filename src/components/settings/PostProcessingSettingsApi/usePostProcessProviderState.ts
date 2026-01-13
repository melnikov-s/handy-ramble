import { useCallback, useEffect, useMemo, useState } from "react";
import { useSettings } from "../../../hooks/useSettings";
import type { LLMProvider, LLMModel } from "@/bindings";
import type { ModelOption } from "./types";
import type { DropdownOption } from "../../ui/Dropdown";
import { commands } from "@/bindings";

type PostProcessProviderState = {
  enabled: boolean;
  providerOptions: DropdownOption[];
  selectedProviderId: string;
  selectedProvider: LLMProvider | undefined;
  isCustomProvider: boolean;
  isAppleProvider: boolean;
  baseUrl: string;
  handleBaseUrlChange: (value: string) => void;
  isBaseUrlUpdating: boolean;
  apiKey: string;
  handleApiKeyChange: (value: string) => void;
  isApiKeyUpdating: boolean;
  model: string;
  handleModelChange: (value: string) => void;
  modelOptions: ModelOption[];
  isModelUpdating: boolean;
  isFetchingModels: boolean;
  handleProviderSelect: (providerId: string) => void;
  handleModelSelect: (value: string) => void;
  handleModelCreate: (value: string) => void;
};

const APPLE_PROVIDER_ID = "apple_intelligence";

export const usePostProcessProviderState = (): PostProcessProviderState => {
  const { settings, isUpdating } = useSettings();
  const [selectedProviderId, setSelectedProviderId] = useState<string>("");

  // Use unified llm_providers instead of deprecated post_process_providers
  const providers = settings?.llm_providers || [];

  // Enable state now uses coherent_enabled
  const enabled = settings?.coherent_enabled || false;

  // Initialize selected provider from default_coherent_model
  useEffect(() => {
    if (!selectedProviderId && settings?.default_coherent_model_id) {
      const defaultModel = (settings?.llm_models || []).find(
        (m) => m.id === settings.default_coherent_model_id,
      );
      if (defaultModel) {
        setSelectedProviderId(defaultModel.provider_id);
      } else if (providers.length > 0) {
        setSelectedProviderId(providers[0].id);
      }
    } else if (!selectedProviderId && providers.length > 0) {
      setSelectedProviderId(providers[0].id);
    }
  }, [settings, providers, selectedProviderId]);

  const selectedProvider = useMemo(() => {
    return providers.find((p) => p.id === selectedProviderId) || providers[0];
  }, [providers, selectedProviderId]);

  const isAppleProvider = selectedProvider?.id === APPLE_PROVIDER_ID;
  const isCustomProvider = selectedProvider?.is_custom || false;

  // Use API key from provider directly
  const baseUrl = selectedProvider?.base_url ?? "";
  const apiKey = selectedProvider?.api_key ?? "";

  // Get model from default_coherent_model_id
  const selectedModel = useMemo(() => {
    const models = settings?.llm_models || [];
    const defaultModelId = settings?.default_coherent_model_id;
    if (defaultModelId) {
      const model = models.find((m) => m.id === defaultModelId);
      if (model && model.provider_id === selectedProviderId) {
        return model;
      }
    }
    // Fall back to first model for this provider
    return models.find((m) => m.provider_id === selectedProviderId);
  }, [settings, selectedProviderId]);

  const model = selectedModel?.model_id ?? "";

  const providerOptions = useMemo<DropdownOption[]>(() => {
    return providers.map((provider) => ({
      value: provider.id,
      label: provider.name,
    }));
  }, [providers]);

  const handleProviderSelect = useCallback((providerId: string) => {
    setSelectedProviderId(providerId);
  }, []);

  const handleBaseUrlChange = useCallback(
    async (value: string) => {
      if (!selectedProvider || !selectedProvider.is_custom) return;
      const trimmed = value.trim();
      if (trimmed && trimmed !== baseUrl) {
        try {
          const updatedProvider = { ...selectedProvider, base_url: trimmed };
          await commands.saveLlmProvider(updatedProvider);
        } catch (error) {
          console.error("Failed to update base URL:", error);
        }
      }
    },
    [selectedProvider, baseUrl],
  );

  const handleApiKeyChange = useCallback(
    async (value: string) => {
      const trimmed = value.trim();
      if (trimmed !== apiKey) {
        try {
          await commands.updateProviderApiKey(selectedProviderId, trimmed);
        } catch (error) {
          console.error("Failed to update API key:", error);
        }
      }
    },
    [apiKey, selectedProviderId],
  );

  const handleModelChange = useCallback(
    async (value: string) => {
      // For model changes, we need to find or create a model entry
      const trimmed = value.trim();
      if (trimmed !== model) {
        try {
          // Set as default coherent model
          await commands.setDefaultModel("coherent", trimmed);
        } catch (error) {
          console.error("Failed to update model:", error);
        }
      }
    },
    [model],
  );

  const handleModelSelect = useCallback(
    async (value: string) => {
      const trimmed = value.trim();
      try {
        // Find or create model and set as default
        const models = settings?.llm_models || [];
        let modelEntry = models.find(
          (m) => m.model_id === trimmed && m.provider_id === selectedProviderId,
        );

        if (!modelEntry) {
          // Create new model entry
          const newModel: LLMModel = {
            id: `${selectedProviderId}-${trimmed.replace(/\//g, "-")}`,
            provider_id: selectedProviderId,
            model_id: trimmed,
            display_name: trimmed,
            supports_vision: false,
            enabled: true,
          };
          const result = await commands.saveLlmModel(newModel);
          if (result.status === "ok") {
            modelEntry = result.data;
          }
        }

        if (modelEntry) {
          await commands.setDefaultModel("coherent", modelEntry.id);
        }
      } catch (error) {
        console.error("Failed to select model:", error);
      }
    },
    [settings, selectedProviderId],
  );

  const handleModelCreate = useCallback(
    async (value: string) => {
      await handleModelSelect(value);
    },
    [handleModelSelect],
  );

  const modelOptions = useMemo<ModelOption[]>(() => {
    const seen = new Set<string>();
    const options: ModelOption[] = [];

    const upsert = (value: string | null | undefined) => {
      const trimmed = value?.trim();
      if (!trimmed || seen.has(trimmed)) return;
      seen.add(trimmed);
      options.push({ value: trimmed, label: trimmed });
    };

    // Add models from settings for this provider ONLY if they are enabled
    // AND the provider has an API key
    const configuredProvider = providers.find(
      (p) => p.id === selectedProviderId,
    );
    if (configuredProvider?.api_key) {
      const settingsModels = settings?.llm_models || [];
      for (const m of settingsModels) {
        if (m.provider_id === selectedProviderId && m.enabled) {
          upsert(m.model_id);
        }
      }
    }

    // Ensure current model is in the list
    upsert(model);

    return options;
  }, [settings, selectedProviderId, model]);

  const isBaseUrlUpdating = isUpdating(`llm_provider:${selectedProviderId}`);
  const isApiKeyUpdating = isUpdating(`provider_api_key:${selectedProviderId}`);
  const isModelUpdating = isUpdating(`default_model:coherent`);
  const isFetchingModels = isUpdating(`fetch_models:${selectedProviderId}`);

  return {
    enabled,
    providerOptions,
    selectedProviderId,
    selectedProvider,
    isCustomProvider,
    isAppleProvider,
    baseUrl,
    handleBaseUrlChange,
    isBaseUrlUpdating,
    apiKey,
    handleApiKeyChange,
    isApiKeyUpdating,
    model,
    handleModelChange,
    modelOptions,
    isModelUpdating,
    isFetchingModels,
    handleProviderSelect,
    handleModelSelect,
    handleModelCreate,
  };
};
