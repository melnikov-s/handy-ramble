import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  RefreshCcw,
  RotateCcw,
  ChevronDown,
  ChevronRight,
  Plus,
  Trash2,
} from "lucide-react";
import { commands, PromptMode, PromptCategory } from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { ResetButton } from "../ui/ResetButton";

import { ModelSelect } from "./PostProcessingSettingsApi/ModelSelect";
import { useSettings } from "../../hooks/useSettings";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { AppMappingsSettings } from "./AppMappingsSettings";

export const RambleSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();

  const [categoryPrompts, setCategoryPrompts] = useState<
    Record<string, string>
  >({});
  const [expandedCategory, setExpandedCategory] = useState<string | null>(null);
  const [isAddingCategory, setIsAddingCategory] = useState(false);
  const [newCategoryName, setNewCategoryName] = useState("");
  const [newCategoryIcon, setNewCategoryIcon] = useState("📝");

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

  // Use centralized LLM provider
  const providerId = (settings as any)?.llm_provider_id ?? "gemini";
  const model = (settings as any)?.ramble_model ?? "";
  const providers = settings?.post_process_providers ?? [];

  const useVisionModel = (settings as any)?.ramble_use_vision_model ?? false;
  const visionModel = (settings as any)?.ramble_vision_model ?? "";

  const selectedProvider = providers.find((p) => p.id === providerId);

  // Sync category prompts from settings
  useEffect(() => {
    const categories = settings?.prompt_categories ?? [];
    const prompts: Record<string, string> = {};
    categories.forEach((cat: PromptCategory) => {
      prompts[cat.id] = cat.prompt;
    });
    setCategoryPrompts(prompts);
  }, [settings?.prompt_categories]);

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

  const handlePromptModeChange = async (mode: PromptMode) => {
    try {
      await commands.changePromptModeSetting(mode);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change prompt mode:", error);
    }
  };

  const handleCategoryPromptChange = (
    categoryId: string,
    newPrompt: string,
  ) => {
    setCategoryPrompts((prev) => ({ ...prev, [categoryId]: newPrompt }));
  };

  const handleCategoryPromptBlur = async (categoryId: string) => {
    const originalPrompt = settings?.prompt_categories?.find(
      (c: PromptCategory) => c.id === categoryId,
    )?.prompt;
    const currentPrompt = categoryPrompts[categoryId];

    if (currentPrompt && currentPrompt.trim() !== originalPrompt?.trim()) {
      try {
        await commands.updatePromptCategory(categoryId, currentPrompt);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update category prompt:", error);
      }
    }
  };

  const handleResetCategoryPrompt = async (categoryId: string) => {
    setIsUpdating(true);
    try {
      const result = await commands.resetPromptCategoryToDefault(categoryId);
      if (result.status === "ok") {
        setCategoryPrompts((prev) => ({ ...prev, [categoryId]: result.data }));
        await refreshSettings();
      }
    } catch (error) {
      console.error("Failed to reset category prompt:", error);
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

  const handleAddCategory = async () => {
    if (!newCategoryName.trim()) return;

    setIsUpdating(true);
    try {
      const result = await (commands as any).addPromptCategory(
        newCategoryName,
        newCategoryIcon || "📝",
        "Enter your custom prompt here.\n\n---\n\nSelected text (may be empty):\n${selection}\n\nInput transcript:\n${output}",
      );
      if (result.status === "ok") {
        await refreshSettings();
        setIsAddingCategory(false);
        setNewCategoryName("");
        setNewCategoryIcon("📝");
        // Expand the newly created category
        setExpandedCategory(result.data.id);
      }
    } catch (error) {
      console.error("Failed to add category:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  const handleDeleteCategory = async (categoryId: string) => {
    setIsUpdating(true);
    try {
      await (commands as any).deletePromptCategory(categoryId);
      await refreshSettings();
      if (expandedCategory === categoryId) {
        setExpandedCategory(null);
      }
    } catch (error) {
      console.error("Failed to delete category:", error);
    } finally {
      setIsUpdating(false);
    }
  };

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

      <SettingsGroup title={t("settings.ramble.llm.title", "AI Model")}>
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
        title={t("settings.ramble.categories.title", "Context-Aware Prompts")}
      >
        <div className="px-4 py-3 text-sm text-mid-gray">
          <p>
            {t(
              "settings.ramble.categories.description",
              "Different prompts are used based on your current application. Dynamic mode auto-detects the app; other modes apply the selected prompt regardless of the active app.",
            )}
          </p>
        </div>

        <SettingContainer
          title={t("settings.ramble.mode.title", "Prompt Mode")}
          description={t(
            "settings.ramble.mode.description",
            "Choose how prompts are selected. Dynamic auto-detects the frontmost app.",
          )}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <select
            value={settings?.prompt_mode ?? "dynamic"}
            onChange={(e) =>
              handlePromptModeChange(e.target.value as PromptMode)
            }
            className="px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
          >
            <option value="dynamic">
              🔄 {t("settings.ramble.mode.dynamic", "Dynamic")}
            </option>
            <option value="development">
              💻 {t("settings.ramble.mode.development", "Development")}
            </option>
            <option value="conversation">
              💬 {t("settings.ramble.mode.conversation", "Conversation")}
            </option>
            <option value="writing">
              ✍️ {t("settings.ramble.mode.writing", "Writing")}
            </option>
            <option value="email">
              📧 {t("settings.ramble.mode.email", "Email")}
            </option>
          </select>
        </SettingContainer>

        {(settings?.prompt_categories ?? []).map((category: PromptCategory) => (
          <div key={category.id} className="border-t border-mid-gray/10">
            <button
              onClick={() =>
                setExpandedCategory(
                  expandedCategory === category.id ? null : category.id,
                )
              }
              className="w-full px-4 py-3 flex items-center justify-between hover:bg-mid-gray/5 transition-colors"
            >
              <div className="flex items-center gap-2">
                <span className="text-lg">{category.icon}</span>
                <span className="font-medium">{category.name}</span>
                {category.is_builtin && (
                  <span className="text-xs text-mid-gray bg-mid-gray/10 px-1.5 py-0.5 rounded">
                    {t("settings.ramble.categories.builtin", "Built-in")}
                  </span>
                )}
              </div>
              {expandedCategory === category.id ? (
                <ChevronDown className="h-4 w-4 text-mid-gray" />
              ) : (
                <ChevronRight className="h-4 w-4 text-mid-gray" />
              )}
            </button>

            {expandedCategory === category.id && (
              <div className="px-4 pb-4 space-y-2">
                <textarea
                  value={categoryPrompts[category.id] ?? ""}
                  onChange={(e) =>
                    handleCategoryPromptChange(category.id, e.target.value)
                  }
                  onBlur={() => handleCategoryPromptBlur(category.id)}
                  placeholder={t(
                    "settings.ramble.categories.promptPlaceholder",
                    "Enter prompt instructions...",
                  )}
                  className="w-full min-h-[200px] p-3 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary resize-y font-mono"
                />
                <div className="flex items-center justify-between text-xs text-mid-gray">
                  <span>
                    {t(
                      "settings.ramble.categories.variables",
                      "Variables: ${output}, ${selection}, ${application}, ${category}",
                    )}
                  </span>
                  <div className="flex items-center gap-2">
                    {category.is_builtin && (
                      <button
                        onClick={() => handleResetCategoryPrompt(category.id)}
                        disabled={isUpdating}
                        className="flex items-center gap-1 px-2 py-1 bg-mid-gray/10 hover:bg-mid-gray/20 rounded transition-colors disabled:opacity-50"
                      >
                        <RotateCcw className="h-3 w-3" />
                        {t("settings.ramble.categories.reset", "Reset")}
                      </button>
                    )}
                    {!category.is_builtin && (
                      <button
                        onClick={() => handleDeleteCategory(category.id)}
                        disabled={isUpdating}
                        className="flex items-center gap-1 px-2 py-1 text-red-500 bg-red-500/10 hover:bg-red-500/20 rounded transition-colors disabled:opacity-50"
                      >
                        <Trash2 className="h-3 w-3" />
                        {t("settings.ramble.categories.delete", "Delete")}
                      </button>
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>
        ))}

        {/* Add New Category */}
        <div className="border-t border-mid-gray/10 px-4 py-3">
          {isAddingCategory ? (
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  value={newCategoryIcon}
                  onChange={(e) => setNewCategoryIcon(e.target.value)}
                  placeholder="📝"
                  className="w-12 px-2 py-2 text-center text-lg bg-background border border-mid-gray/30 rounded-lg focus:outline-none focus:border-logo-primary"
                  title={t(
                    "settings.ramble.categories.iconHint",
                    "Use Cmd+Ctrl+Space for emoji picker",
                  )}
                />
                <input
                  type="text"
                  value={newCategoryName}
                  onChange={(e) => setNewCategoryName(e.target.value)}
                  placeholder={t(
                    "settings.ramble.categories.namePlaceholder",
                    "Category name...",
                  )}
                  className="flex-1 px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
                  autoFocus
                />
              </div>
              <div className="flex items-center justify-between">
                <span className="text-xs text-mid-gray">
                  {t(
                    "settings.ramble.categories.emojiHint",
                    "Tip: Press Cmd+Ctrl+Space to open emoji picker",
                  )}
                </span>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => {
                      setIsAddingCategory(false);
                      setNewCategoryName("");
                      setNewCategoryIcon("📝");
                    }}
                    className="px-3 py-1.5 text-sm text-mid-gray hover:text-foreground transition-colors"
                  >
                    {t("common.cancel", "Cancel")}
                  </button>
                  <button
                    onClick={handleAddCategory}
                    disabled={!newCategoryName.trim() || isUpdating}
                    className="flex items-center gap-1 px-3 py-1.5 bg-logo-primary text-white rounded-lg text-sm hover:bg-logo-primary/90 transition-colors disabled:opacity-50"
                  >
                    <Plus className="h-4 w-4" />
                    {t("settings.ramble.categories.create", "Create")}
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <button
              onClick={() => setIsAddingCategory(true)}
              className="flex items-center gap-2 text-sm text-logo-primary hover:text-logo-primary/80 transition-colors"
            >
              <Plus className="h-4 w-4" />
              {t("settings.ramble.categories.addNew", "Add Custom Category")}
            </button>
          )}
        </div>
      </SettingsGroup>

      {/* App Mappings Section (only visible when Dynamic mode is selected) */}
      {settings?.prompt_mode === "dynamic" && <AppMappingsSettings />}
    </div>
  );
};
