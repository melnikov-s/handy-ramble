import React from "react";
import { useSettings } from "../../hooks/useSettings";
import { Dropdown, DropdownOption } from "./Dropdown";

interface ModelsDropdownProps {
  selectedValue: string | null;
  onSelect: (value: string | null) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  direction?: "up" | "down";
  allowDefault?: boolean;
  defaultLabel?: string;
}

export const ModelsDropdown: React.FC<ModelsDropdownProps> = ({
  selectedValue,
  onSelect,
  placeholder = "Select a model",
  disabled = false,
  className = "",
  direction = "down",
  allowDefault = false,
  defaultLabel = "Use Default",
}) => {
  const { settings, isLoading } = useSettings();

  // Filter for enabled models, deduplicate, and ensure provider is configured
  const providers = settings?.llm_providers || [];
  const models = settings?.llm_models || [];

  const configuredProviderIds = new Set(
    providers
      .filter((p) => {
        const extProvider = p as typeof p & { auth_method?: string };
        // Provider is configured if it has API key OR uses OAuth
        return (
          (p.api_key && p.api_key.trim() !== "") ||
          extProvider.auth_method === "oauth"
        );
      })
      .map((p) => p.id),
  );
  const enabledModels = models.filter(
    (m) =>
      m.enabled === true &&
      m.provider_id &&
      configuredProviderIds.has(m.provider_id),
  );

  // Format options with provider / model label
  const modelOptions: DropdownOption[] = enabledModels
    .map((m) => {
      const provider = providers.find((p) => p.id === m.provider_id);
      return {
        value: m.id,
        label: `${provider?.name || m.provider_id} / ${m.model_id}`,
      };
    })
    // Deduplicate as safeguard
    .filter((v, i, a) => a.findIndex((t) => t.value === v.value) === i);

  if (allowDefault) {
    modelOptions.unshift({
      value: "__default__",
      label: defaultLabel,
    });
  }

  const getPlaceholder = () => {
    if (isLoading) return "Loading...";
    if (modelOptions.length === 0) return "No models configured";
    return placeholder;
  };

  const handleSelect = (value: string) => {
    if (value === "__default__") {
      onSelect(null);
    } else {
      onSelect(value);
    }
  };

  return (
    <Dropdown
      selectedValue={
        selectedValue === null && allowDefault ? "__default__" : selectedValue
      }
      options={modelOptions}
      onSelect={handleSelect}
      disabled={disabled || isLoading}
      placeholder={getPlaceholder()}
      className={className}
      direction={direction}
    />
  );
};
