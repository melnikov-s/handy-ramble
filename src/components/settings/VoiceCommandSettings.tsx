import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Terminal,
  Mic,
  RefreshCcw,
  RotateCcw,
  Plus,
  Pencil,
  Trash2,
} from "lucide-react";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { ModelSelect } from "./PostProcessingSettingsApi/ModelSelect";
import { ResetButton } from "../ui/ResetButton";
import { useSettings } from "../../hooks/useSettings";
import { CommandEditorModal } from "./CommandEditorModal";
import { VoiceCommand } from "@/bindings";

// Define a local type for the commands we need until bindings regenerate
interface VoiceCommandCommands {
  changeVoiceCommandDefaultModelSetting: (model: string) => Promise<any>;
  resetVoiceCommandsToDefault: () => Promise<any>;
  deleteVoiceCommand: (commandId: string) => Promise<any>;
}

// Import commands with type assertion
import { commands as rawCommands } from "@/bindings";
const commands = rawCommands as unknown as VoiceCommandCommands &
  typeof rawCommands;

export const VoiceCommandSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();

  const [isUpdating, setIsUpdating] = useState(false);
  const [isFetchingModels, setIsFetchingModels] = useState(false);

  // Access settings with type safety
  const defaultModel =
    (settings as any)?.voice_command_default_model ?? "gemini-2.0-flash";
  const voiceCommands = (settings as any)?.voice_commands ?? [];
  const voiceCommandBinding =
    settings?.bindings?.["voice_command"]?.current_binding ?? "right_command";

  // Use centralized LLM provider
  const providerId = (settings as any)?.llm_provider_id ?? "gemini";

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

  const handleModelChange = async (newModel: string) => {
    setIsUpdating(true);
    try {
      await commands.changeVoiceCommandDefaultModelSetting(newModel);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change voice command model:", error);
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

  const handleResetCommands = async () => {
    setIsUpdating(true);
    try {
      await commands.resetVoiceCommandsToDefault();
      await refreshSettings();
    } catch (error) {
      console.error("Failed to reset voice commands:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  // Modal state for command editor
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingCommand, setEditingCommand] = useState<VoiceCommand | null>(
    null,
  );

  const handleAddCommand = () => {
    setEditingCommand(null);
    setIsModalOpen(true);
  };

  const handleEditCommand = (cmd: VoiceCommand) => {
    setEditingCommand(cmd);
    setIsModalOpen(true);
  };

  const handleDeleteCommand = async (commandId: string) => {
    if (
      !confirm(
        t("settings.voiceCommands.confirmDelete", "Delete this command?"),
      )
    ) {
      return;
    }
    setIsUpdating(true);
    try {
      await commands.deleteVoiceCommand(commandId);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to delete voice command:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  const handleModalSave = async () => {
    await refreshSettings();
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup
        title={t("settings.voiceCommands.title", "Voice Commands")}
      >
        <div className="px-4 py-3 text-sm text-mid-gray">
          <p className="mb-2">
            <strong>
              {t(
                "settings.voiceCommands.howItWorks.title",
                "Control your computer with voice:",
              )}
            </strong>
          </p>
          <ul className="list-disc list-inside space-y-1">
            <li>
              {t(
                "settings.voiceCommands.howItWorks.activate",
                `Press ${voiceCommandBinding} to activate command mode`,
              ).replace("${voiceCommandBinding}", voiceCommandBinding)}
            </li>
            <li>
              {t(
                "settings.voiceCommands.howItWorks.speak",
                "Speak a command like 'open Chrome' or 'search for weather'",
              )}
            </li>
            <li>
              {t(
                "settings.voiceCommands.howItWorks.execute",
                "AI interprets your command and executes the appropriate action",
              )}
              \n{" "}
            </li>
          </ul>
        </div>
      </SettingsGroup>

      {
        <SettingsGroup
          title={t("settings.voiceCommands.model.title", "AI Model")}
        >
          <SettingContainer
            title={t(
              "settings.voiceCommands.defaultModel.title",
              "Default Model",
            )}
            description={t(
              "settings.voiceCommands.defaultModel.description",
              "The AI model used to interpret and execute voice commands. Fast models are recommended.",
            )}
            descriptionMode="tooltip"
            layout="stacked"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <ModelSelect
                value={defaultModel}
                options={modelOptions}
                disabled={isUpdating}
                isLoading={isFetchingModels}
                placeholder={t(
                  "settings.voiceCommands.model.placeholder",
                  "Select a model",
                )}
                onSelect={handleModelChange}
                onCreate={handleModelChange}
                onBlur={() => {}}
                className="flex-1 min-w-[380px]"
              />
              <ResetButton
                onClick={handleRefreshModels}
                disabled={isFetchingModels}
                ariaLabel={t(
                  "settings.voiceCommands.model.refresh",
                  "Refresh models",
                )}
              >
                <RefreshCcw
                  className={`h-4 w-4 ${isFetchingModels ? "animate-spin" : ""}`}
                />
              </ResetButton>
            </div>
          </SettingContainer>

          <div className="px-4 py-3 text-xs text-mid-gray bg-mid-gray/5 rounded-lg mx-4 mb-4">
            <p>
              <strong>💡 Tip:</strong>{" "}
              {t(
                "settings.voiceCommands.model.tip",
                "Individual commands can override this model if they need more reasoning capability.",
              )}
            </p>
          </div>
        </SettingsGroup>
      }

      {voiceCommands.length > 0 && (
        <SettingsGroup
          title={t(
            "settings.voiceCommands.commands.title",
            "Available Commands",
          )}
        >
          <div className="divide-y divide-mid-gray/10">
            {voiceCommands.map((cmd: any) => (
              <div
                key={cmd.id}
                className="px-4 py-3 flex items-center gap-3 group"
              >
                <div
                  className={`p-2 rounded-lg ${cmd.command_type === "bespoke" ? "bg-purple-500/10" : "bg-blue-500/10"}`}
                >
                  {cmd.command_type === "bespoke" ? (
                    <Terminal className="h-4 w-4 text-purple-500" />
                  ) : (
                    <Mic className="h-4 w-4 text-blue-500" />
                  )}
                </div>
                <div className="flex-1">
                  <div className="font-medium text-sm">{cmd.name}</div>
                  <div className="text-xs text-mid-gray">
                    {cmd.phrases?.slice(0, 3).join(", ")}
                    {cmd.phrases?.length > 3 &&
                      ` +${cmd.phrases.length - 3} more`}
                  </div>
                </div>
                {cmd.is_builtin && (
                  <span className="text-xs text-mid-gray bg-mid-gray/10 px-1.5 py-0.5 rounded">
                    {t("settings.voiceCommands.commands.builtin", "Built-in")}
                  </span>
                )}
                {/* Edit/Delete buttons */}
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    onClick={() => handleEditCommand(cmd)}
                    className="p-1.5 hover:bg-mid-gray/20 rounded transition-colors"
                    title={t("common.edit", "Edit")}
                  >
                    <Pencil className="h-3.5 w-3.5 text-mid-gray" />
                  </button>
                  {!cmd.is_builtin && (
                    <button
                      onClick={() => handleDeleteCommand(cmd.id)}
                      disabled={isUpdating}
                      className="p-1.5 hover:bg-red-500/20 rounded transition-colors"
                      title={t("common.delete", "Delete")}
                    >
                      <Trash2 className="h-3.5 w-3.5 text-red-400" />
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
          <div className="px-4 py-3 border-t border-mid-gray/10">
            <button
              onClick={handleResetCommands}
              disabled={isUpdating}
              className="flex items-center gap-2 text-sm text-mid-gray hover:text-white transition-colors"
            >
              <RotateCcw className="h-4 w-4" />
              {t("settings.voiceCommands.commands.reset", "Reset to Defaults")}
            </button>
          </div>
        </SettingsGroup>
      )}

      {/* Add Command Button */}
      {
        <div className="flex justify-center">
          <button
            onClick={handleAddCommand}
            className="flex items-center gap-2 px-4 py-2 text-sm bg-logo-primary/20 hover:bg-logo-primary/30 text-logo-primary rounded-lg transition-colors"
          >
            <Plus className="h-4 w-4" />
            {t("settings.voiceCommands.addCommand", "Add Command")}
          </button>
        </div>
      }

      {/* Command Editor Modal */}
      <CommandEditorModal
        isOpen={isModalOpen}
        onClose={() => setIsModalOpen(false)}
        onSave={handleModalSave}
        command={editingCommand}
      />
    </div>
  );
};
