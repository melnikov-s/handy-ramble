import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { commands } from "@/bindings";

interface FillerWordFilterProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const DEFAULT_PATTERN = String.raw`\b(u+[hm]+|a+h+|e+r+m?|m+h?m+|h+m+)\b[,\s]*`;

export const FillerWordFilter: React.FC<FillerWordFilterProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { settings, refreshSettings } = useSettings();
    const [error, setError] = useState<string | null>(null);
    const [isUpdating, setIsUpdating] = useState(false);

    const currentPattern = settings?.filler_word_filter ?? null;
    const isEnabled = currentPattern !== null && currentPattern !== "";

    const handleToggle = async (enabled: boolean) => {
      setIsUpdating(true);
      setError(null);
      try {
        const newPattern = enabled ? DEFAULT_PATTERN : null;
        const result = await commands.changeFillerWordFilterSetting(newPattern);
        if (result.status === "error") {
          setError(result.error);
        } else {
          await refreshSettings();
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setIsUpdating(false);
      }
    };

    const handlePatternChange = async (
      e: React.ChangeEvent<HTMLInputElement>,
    ) => {
      const newPattern = e.target.value;
      setIsUpdating(true);
      setError(null);
      try {
        const result = await commands.changeFillerWordFilterSetting(
          newPattern || null,
        );
        if (result.status === "error") {
          setError(result.error);
        } else {
          await refreshSettings();
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setIsUpdating(false);
      }
    };

    const handleReset = async () => {
      setIsUpdating(true);
      setError(null);
      try {
        const result =
          await commands.changeFillerWordFilterSetting(DEFAULT_PATTERN);
        if (result.status === "error") {
          setError(result.error);
        } else {
          await refreshSettings();
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setIsUpdating(false);
      }
    };

    return (
      <SettingContainer
        title={t(
          "settings.advanced.fillerWordFilter.title",
          "Filter Filler Words",
        )}
        description={t(
          "settings.advanced.fillerWordFilter.description",
          "Remove filler words like 'um', 'uh', 'hmm' from raw transcriptions using a regex pattern.",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <div className="space-y-3">
          <ToggleSwitch
            checked={isEnabled}
            onChange={handleToggle}
            disabled={isUpdating}
            label={t(
              "settings.advanced.fillerWordFilter.enable",
              "Enable filter",
            )}
            description={t(
              "settings.advanced.fillerWordFilter.toggleDescription",
              "Filter out filler words from raw transcriptions",
            )}
          />
          {isEnabled && (
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Input
                  type="text"
                  className="flex-1 font-mono text-sm"
                  value={currentPattern || ""}
                  onChange={handlePatternChange}
                  placeholder={t(
                    "settings.advanced.fillerWordFilter.placeholder",
                    "Regex pattern",
                  )}
                  variant="compact"
                  disabled={isUpdating}
                />
                <button
                  onClick={handleReset}
                  disabled={isUpdating || currentPattern === DEFAULT_PATTERN}
                  className="text-xs text-mid-gray hover:text-white transition-colors disabled:opacity-50"
                  title={t(
                    "settings.advanced.fillerWordFilter.reset",
                    "Reset to default",
                  )}
                >
                  ↺
                </button>
              </div>
              {error && <p className="text-xs text-red-400">{error}</p>}
              <p className="text-xs text-mid-gray">
                {t(
                  "settings.advanced.fillerWordFilter.hint",
                  "Default pattern matches: um, uh, ah, hmm, mhm, er, erm",
                )}
              </p>
            </div>
          )}
        </div>
      </SettingContainer>
    );
  },
);
