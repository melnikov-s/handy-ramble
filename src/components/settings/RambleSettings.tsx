import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  RotateCcw,
  ChevronDown,
  ChevronRight,
  Plus,
  Trash2,
} from "lucide-react";
import {
  commands,
  PromptMode,
  PromptCategory,
  DefaultModels,
} from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { ModelsDropdown } from "../ui/ModelsDropdown";

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

  // Default models state for current selection
  const [defaultModels, setDefaultModels] = useState<DefaultModels | null>(
    null,
  );
  const [isUpdating, setIsUpdating] = useState(false);

  // Load defaults on mount
  useEffect(() => {
    const loadDefaults = async () => {
      try {
        const defaults = await commands.getDefaultModels();
        setDefaultModels(defaults);
      } catch (error) {
        console.error("Failed to load default models:", error);
      }
    };
    loadDefaults();
  }, []);

  // Sync category prompts from settings
  useEffect(() => {
    const categories = settings?.prompt_categories ?? [];
    const prompts: Record<string, string> = {};
    categories.forEach((cat: PromptCategory) => {
      prompts[cat.id] = cat.prompt;
    });
    setCategoryPrompts(prompts);
  }, [settings?.prompt_categories]);

  const handleModelChange = async (modelId: string) => {
    setIsUpdating(true);
    try {
      await commands.setDefaultModel("coherent", modelId);
      // Reload defaults
      const defaults = await commands.getDefaultModels();
      setDefaultModels(defaults);
    } catch (error) {
      console.error("Failed to change coherent model:", error);
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
          title={t("settings.ramble.aiModel.title", "AI Model")}
          description={t(
            "settings.ramble.aiModel.description",
            "Select the AI model to use for Ramble to Coherent transformation.",
          )}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={true}
        >
          <ModelsDropdown
            selectedValue={defaultModels?.coherent || null}
            onSelect={handleModelChange}
            disabled={isUpdating}
            className="min-w-[280px]"
          />
        </SettingContainer>

        <div className="px-4 py-2 text-xs text-mid-gray">
          <p>
            {t(
              "settings.ramble.model.hint",
              "Models can be enabled in Settings → AI Providers. Only enabled models appear here.",
            )}
          </p>
        </div>
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
              {t("settings.ramble.mode.dynamic", "Dynamic")}
            </option>
            <option value="low">
              ▁ {t("settings.ramble.mode.low", "Low")}
            </option>
            <option value="medium">
              ▃ {t("settings.ramble.mode.medium", "Medium")}
            </option>
            <option value="high">
              ▅ {t("settings.ramble.mode.high", "High")}
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
