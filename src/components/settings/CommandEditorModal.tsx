import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X, Terminal } from "lucide-react";

import {
  commands as rawCommands,
  VoiceCommand,
  VoiceCommandType,
  ScriptType,
} from "@/bindings";

// Type assertion for new commands
interface VoiceCommands {
  addVoiceCommand: (command: VoiceCommand) => Promise<any>;
  updateVoiceCommand: (command: VoiceCommand) => Promise<any>;
}
const commands = rawCommands as unknown as VoiceCommands & typeof rawCommands;

interface CommandEditorModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: () => void;
  command?: VoiceCommand | null; // null = create mode, existing = edit mode
}

export const CommandEditorModal: React.FC<CommandEditorModalProps> = ({
  isOpen,
  onClose,
  onSave,
  command,
}) => {
  const { t } = useTranslation();
  const isEditing = !!command;

  // Form state
  const [name, setName] = useState("");
  const [phrases, setPhrases] = useState("");
  const [commandType, setCommandType] = useState<VoiceCommandType>("custom");
  const [description, setDescription] = useState("");
  const [scriptType, setScriptType] = useState<ScriptType>("shell");
  const [script, setScript] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Populate form when editing
  useEffect(() => {
    if (command) {
      setName(command.name);
      setPhrases(command.phrases.join(", "));
      setCommandType(command.command_type);
      setDescription(command.description || "");
      setScriptType(command.script_type || "shell");
      setScript(command.script || "");
    } else {
      // Reset for new command
      setName("");
      setPhrases("");
      setCommandType("custom");
      setDescription("");
      setScriptType("shell");
      setScript("");
    }
    setError(null);
  }, [command, isOpen]);

  const handleSave = async () => {
    // Validate
    if (!name.trim()) {
      setError("Name is required");
      return;
    }
    if (!phrases.trim()) {
      setError("At least one trigger phrase is required");
      return;
    }
    if (commandType === "custom" && !script.trim()) {
      setError("Script is required for custom commands");
      return;
    }

    setIsSaving(true);
    setError(null);

    try {
      const commandData: VoiceCommand = {
        id: command?.id || name.toLowerCase().replace(/\s+/g, "_"),
        name: name.trim(),
        phrases: phrases
          .split(",")
          .map((p) => p.trim())
          .filter((p) => p),
        command_type: commandType,
        description: description.trim() || null,
        script_type: scriptType,
        script: commandType === "custom" ? script.trim() : null,
        model_override: null, // Removed - using centralized model
        is_builtin: false,
      };

      if (isEditing) {
        await commands.updateVoiceCommand(commandData);
      } else {
        await commands.addVoiceCommand(commandData);
      }

      onSave();
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSaving(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
      <div className="bg-[#1a1a1a] rounded-xl shadow-2xl w-full max-w-lg mx-4 border border-mid-gray/30">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-mid-gray/20">
          <h2 className="text-lg font-semibold">
            {isEditing
              ? t("commandEditor.editTitle", "Edit Command")
              : t("commandEditor.createTitle", "New Command")}
          </h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-mid-gray/20 rounded-lg transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Body */}
        <div className="px-6 py-4 space-y-4 max-h-[60vh] overflow-y-auto">
          {error && (
            <div className="text-sm text-red-400 bg-red-900/20 px-3 py-2 rounded-lg">
              {error}
            </div>
          )}

          {/* Name */}
          <div>
            <label className="block text-sm font-medium mb-1">
              {t("commandEditor.name", "Command Name")}
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t(
                "commandEditor.namePlaceholder",
                "e.g., Open Application",
              )}
              className="w-full px-3 py-2 bg-black/20 border border-mid-gray/30 rounded-lg focus:outline-none focus:border-logo-primary"
            />
          </div>

          {/* Trigger Phrases */}
          <div>
            <label className="block text-sm font-medium mb-1">
              {t("commandEditor.phrases", "Trigger Phrases")}
            </label>
            <input
              type="text"
              value={phrases}
              onChange={(e) => setPhrases(e.target.value)}
              placeholder={t(
                "commandEditor.phrasesPlaceholder",
                "open, launch, start",
              )}
              className="w-full px-3 py-2 bg-black/20 border border-mid-gray/30 rounded-lg focus:outline-none focus:border-logo-primary"
            />
            <p className="text-xs text-mid-gray mt-1">
              {t(
                "commandEditor.phrasesHint",
                "Separate multiple phrases with commas",
              )}
            </p>
          </div>

          {/* Command Type - For new commands, just show as custom */}
          <div>
            <label className="block text-sm font-medium mb-2">
              {t("commandEditor.type", "Command Type")}
            </label>
            <div className="px-4 py-3 rounded-lg border border-mid-gray/30 bg-black/20">
              <div className="flex items-center gap-2">
                <Terminal className="w-4 h-4 text-purple-400" />
                <span className="text-purple-400">
                  {t("commandEditor.typeCustom", "Custom Script")}
                </span>
              </div>
              <p className="text-xs text-mid-gray mt-2">
                {t(
                  "commandEditor.customHint",
                  "Runs your script when triggered",
                )}
              </p>
            </div>
          </div>

          {/* Description - optional for all commands */}
          <div>
            <label className="block text-sm font-medium mb-1">
              {t("commandEditor.description", "Description")}{" "}
              <span className="text-mid-gray">(optional)</span>
            </label>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t(
                "commandEditor.descriptionPlaceholder",
                "Describe what this command does...",
              )}
              rows={2}
              className="w-full px-3 py-2 bg-black/20 border border-mid-gray/30 rounded-lg focus:outline-none focus:border-logo-primary resize-none"
            />
          </div>

          {/* Script */}
          <>
            <div>
              <label className="block text-sm font-medium mb-2">
                {t("commandEditor.scriptType", "Script Type")}
              </label>
              <div className="flex gap-2">
                <button
                  onClick={() => setScriptType("shell")}
                  className={`px-4 py-2 rounded-lg border transition-colors ${
                    scriptType === "shell"
                      ? "border-logo-primary bg-logo-primary/10"
                      : "border-mid-gray/30 hover:border-mid-gray/50"
                  }`}
                >
                  Shell
                </button>
                <button
                  onClick={() => setScriptType("apple_script")}
                  className={`px-4 py-2 rounded-lg border transition-colors ${
                    scriptType === "apple_script"
                      ? "border-logo-primary bg-logo-primary/10"
                      : "border-mid-gray/30 hover:border-mid-gray/50"
                  }`}
                >
                  AppleScript
                </button>
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">
                {t("commandEditor.script", "Script")}
              </label>
              <textarea
                value={script}
                onChange={(e) => setScript(e.target.value)}
                placeholder={
                  scriptType === "shell"
                    ? "#!/bin/bash\necho 'Hello'"
                    : 'tell application "Finder" to activate'
                }
                rows={5}
                className="w-full px-3 py-2 bg-black/20 border border-mid-gray/30 rounded-lg focus:outline-none focus:border-logo-primary resize-none font-mono text-sm"
              />
            </div>
          </>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 px-6 py-4 border-t border-mid-gray/20">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-mid-gray hover:text-white transition-colors"
          >
            {t("common.cancel", "Cancel")}
          </button>
          <button
            onClick={handleSave}
            disabled={isSaving}
            className="px-4 py-2 text-sm bg-logo-primary hover:bg-logo-primary/80 rounded-lg transition-colors disabled:opacity-50"
          >
            {isSaving
              ? t("common.saving", "Saving...")
              : isEditing
                ? t("common.save", "Save")
                : t("common.create", "Create")}
          </button>
        </div>
      </div>
    </div>
  );
};
