import React, { useEffect, useState } from "react";
import { commands, LLMModel, LLMProvider } from "@/bindings";
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
  const [llmProviders, setLlmProviders] = useState<LLMProvider[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  const loadModels = async () => {
    try {
      setIsLoading(true);
      const [models, providers] = await Promise.all([
        commands.getLlmModels(),
        commands.getLlmProviders(),
      ]);
      setLlmModels(models);
      setLlmProviders(providers);
    } catch (error) {
      console.error("Failed to load LLM models:", error);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadModels();
  }, []);

  // Filter for enabled models and deduplicate
  const enabledModels = llmModels.filter((m) => m.enabled);

  // Format options with raw provider_id / model_id label
  const modelOptions: DropdownOption[] = enabledModels
    .map((m) => {
      return {
        value: m.id,
        label: `${m.provider_id} / ${m.model_id}`,
      };
    })
    // Deduplicate in frontend as a safeguard
    .filter((v, i, a) => a.findIndex((t) => t.value === v.value) === i);

  return (
    <Dropdown
      selectedValue={selectedValue}
      options={modelOptions}
      onSelect={onSelect}
      disabled={disabled || isLoading || modelOptions.length === 0}
      placeholder={
        isLoading
          ? "Loading models..."
          : modelOptions.length === 0
            ? "No models enabled - check AI Providers"
            : placeholder
      }
      onRefresh={loadModels}
      className={className}
      direction={direction}
    />
  );
};
