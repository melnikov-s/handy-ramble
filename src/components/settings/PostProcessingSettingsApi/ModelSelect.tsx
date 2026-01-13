import React, { useEffect, useState } from "react";
import type { ModelOption } from "./types";
import { Select } from "../../ui/Select";
import { commands, LLMProvider } from "@/bindings";

type ModelSelectProps = {
  value: string;
  options: ModelOption[];
  disabled?: boolean;
  placeholder?: string;
  isLoading?: boolean;
  onSelect: (value: string) => void;
  onCreate: (value: string) => void;
  onBlur: () => void;
  className?: string;
};

export const ModelSelect: React.FC<ModelSelectProps> = React.memo(
  ({
    value,
    options,
    disabled,
    placeholder,
    isLoading,
    onSelect,
    onCreate,
    onBlur,
    className = "flex-1 min-w-[360px]",
  }) => {
    const [providers, setProviders] = useState<LLMProvider[]>([]);

    useEffect(() => {
      const loadProviders = async () => {
        try {
          const p = await commands.getLlmProviders();
          setProviders(p);
        } catch (error) {
          console.error("Failed to load providers in ModelSelect:", error);
        }
      };
      loadProviders();
    }, []);

    const configuredProviderIds = new Set(
      providers.filter((p) => p.api_key).map((p) => p.id),
    );

    // Filter options to only show those belonging to configured providers
    // Note: ModelSelect options are usually just strings (model_id),
    // but the dropdown options in usePostProcessProviderState are already filtered by selectedProviderId.
    // However, for consistency and safety across the app, we can apply filtering if needed.
    // In this specific component, options are already filtered by the hook.

    const handleCreate = (inputValue: string) => {
      const trimmed = inputValue.trim();
      if (!trimmed) return;
      onCreate(trimmed);
    };

    const computedClassName = `text-sm ${className}`;

    return (
      <Select
        className={computedClassName}
        value={value || null}
        options={options}
        onChange={(selected) => onSelect(selected ?? "")}
        onCreateOption={handleCreate}
        onBlur={onBlur}
        placeholder={placeholder}
        disabled={disabled}
        isLoading={isLoading}
        isCreatable
        formatCreateLabel={(input) => `Use "${input}"`}
      />
    );
  },
);

ModelSelect.displayName = "ModelSelect";
