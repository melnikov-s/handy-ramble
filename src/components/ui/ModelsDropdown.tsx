import React, { useEffect, useState } from "react";
import { commands, LLMModel } from "@/bindings";
import { Dropdown, DropdownOption } from "./Dropdown";

interface ModelsDropdownProps {
  selectedValue: string | null;
  onSelect: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  direction?: "up" | "down";
}

export const ModelsDropdown: React.FC<ModelsDropdownProps> = ({
  selectedValue,
  onSelect,
  placeholder = "Select a model",
  disabled = false,
  className = "",
  direction = "down",
}) => {
  const [llmModels, setLlmModels] = useState<LLMModel[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Load models from persisted settings
  const loadModels = async () => {
    try {
      setIsLoading(true);
      const models = await commands.getLlmModels();
      setLlmModels(models);
    } catch (error) {
      console.error("Failed to load LLM models:", error);
    } finally {
      setIsLoading(false);
    }
  };

  // Refresh models from provider APIs
  const handleRefresh = async () => {
    try {
      setIsRefreshing(true);
      const result = await commands.refreshAllModels();
      if (result.status === "ok") {
        setLlmModels(result.data);
      } else {
        console.error("Failed to refresh models:", result.error);
        // Still reload from settings in case some models were saved
        await loadModels();
      }
    } catch (error) {
      console.error("Failed to refresh models:", error);
    } finally {
      setIsRefreshing(false);
    }
  };

  useEffect(() => {
    loadModels();
  }, []);

  // Filter for enabled models and deduplicate
  const enabledModels = llmModels.filter((m) => m.enabled);

  // Format options with provider / model label
  const modelOptions: DropdownOption[] = enabledModels
    .map((m) => ({
      value: m.id,
      label: `${m.provider_id} / ${m.model_id}`,
    }))
    // Deduplicate as safeguard
    .filter((v, i, a) => a.findIndex((t) => t.value === v.value) === i);

  const getPlaceholder = () => {
    if (isLoading) return "Loading...";
    if (isRefreshing) return "Refreshing from providers...";
    if (modelOptions.length === 0) return "No models - click refresh";
    return placeholder;
  };

  return (
    <Dropdown
      selectedValue={selectedValue}
      options={modelOptions}
      onSelect={onSelect}
      disabled={disabled || isLoading || isRefreshing}
      placeholder={getPlaceholder()}
      onRefresh={handleRefresh}
      className={className}
      direction={direction}
    />
  );
};
