import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight, Trash2, Plus } from "lucide-react";
import {
  commands,
  AppCategoryMapping,
  DetectedApp,
  PromptCategory,
} from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { useSettings } from "../../hooks/useSettings";

// Type for apps we can display in the dropdown
interface AppOption {
  bundleId: string;
  name: string;
  source: "installed" | "known" | "detected";
  suggestedCategory?: string;
}

// Fallback categories in case settings haven't loaded yet
const DEFAULT_CATEGORIES = [
  { id: "development", name: "Development", icon: "💻" },
  { id: "conversation", name: "Conversation", icon: "💬" },
  { id: "writing", name: "Writing", icon: "✍️" },
  { id: "email", name: "Email", icon: "📧" },
];

export const AppMappingsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();

  const [isExpanded, setIsExpanded] = useState(false);
  const [availableApps, setAvailableApps] = useState<AppOption[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedApp, setSelectedApp] = useState<AppOption | null>(null);
  const [selectedCategory, setSelectedCategory] = useState("development");
  const [isAdding, setIsAdding] = useState(false);

  // Get current mappings from settings
  const mappings: AppCategoryMapping[] =
    (settings as any)?.app_category_mappings ?? [];
  const detectedApps: DetectedApp[] =
    (settings as any)?.detected_apps_history ?? [];
  const promptCategories: PromptCategory[] = settings?.prompt_categories ?? [];
  const defaultCategoryId: string =
    (settings as any)?.default_category_id ?? "development";

  const handleDefaultCategoryChange = async (categoryId: string) => {
    try {
      await (commands as any).changeDefaultCategorySetting(categoryId);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to change default category:", error);
    }
  };

  // Load available apps when expanded
  useEffect(() => {
    if (isExpanded && availableApps.length === 0) {
      loadApps();
    }
  }, [isExpanded]);

  const loadApps = async () => {
    setIsLoading(true);
    try {
      const apps: AppOption[] = [];
      const seenBundleIds = new Set<string>();

      // Load detected apps first (most relevant)
      for (const app of detectedApps) {
        if (!seenBundleIds.has(app.bundle_identifier)) {
          apps.push({
            bundleId: app.bundle_identifier,
            name: app.display_name,
            source: "detected",
          });
          seenBundleIds.add(app.bundle_identifier);
        }
      }

      // Load known apps
      try {
        const knownResult = await (commands as any).getKnownApplications();
        if (Array.isArray(knownResult)) {
          for (const app of knownResult) {
            if (!seenBundleIds.has(app.bundle_id)) {
              apps.push({
                bundleId: app.bundle_id,
                name: app.name,
                source: "known",
                suggestedCategory: app.suggested_category,
              });
              seenBundleIds.add(app.bundle_id);
            }
          }
        }
      } catch (e) {
        console.error("Failed to load known apps:", e);
      }

      // Load installed apps
      try {
        const installedResult = await (
          commands as any
        ).getInstalledApplications();
        if (Array.isArray(installedResult)) {
          for (const app of installedResult) {
            if (!seenBundleIds.has(app.bundle_id)) {
              apps.push({
                bundleId: app.bundle_id,
                name: app.name,
                source: "installed",
              });
              seenBundleIds.add(app.bundle_id);
            }
          }
        }
      } catch (e) {
        console.error("Failed to load installed apps:", e);
      }

      // Sort by name
      apps.sort((a, b) => a.name.localeCompare(b.name));
      setAvailableApps(apps);
    } catch (error) {
      console.error("Failed to load apps:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleAddMapping = async () => {
    if (!selectedApp) return;

    setIsAdding(true);
    try {
      await (commands as any).setAppCategoryMapping(
        selectedApp.bundleId,
        selectedApp.name,
        selectedCategory,
      );
      await refreshSettings();
      setSelectedApp(null);
      setSearchTerm("");
    } catch (error) {
      console.error("Failed to add mapping:", error);
    } finally {
      setIsAdding(false);
    }
  };

  const handleRemoveMapping = async (bundleId: string) => {
    try {
      await (commands as any).removeAppCategoryMapping(bundleId);
      await refreshSettings();
    } catch (error) {
      console.error("Failed to remove mapping:", error);
    }
  };

  const handleCategoryChange = async (
    bundleId: string,
    displayName: string,
    newCategoryId: string,
  ) => {
    try {
      await (commands as any).setAppCategoryMapping(
        bundleId,
        displayName,
        newCategoryId,
      );
      await refreshSettings();
    } catch (error) {
      console.error("Failed to update mapping:", error);
    }
  };

  // Filter apps based on search and exclude already mapped apps
  const filteredApps = availableApps.filter((app) => {
    const isMapped = mappings.some((m) => m.bundle_identifier === app.bundleId);
    if (isMapped) return false;
    if (!searchTerm) return true;
    return app.name.toLowerCase().includes(searchTerm.toLowerCase());
  });

  const getCategoryInfo = (categoryId: string) => {
    // Check dynamic categories from settings first
    const fromSettings = promptCategories.find((c) => c.id === categoryId);
    if (fromSettings)
      return {
        id: fromSettings.id,
        name: fromSettings.name,
        icon: fromSettings.icon,
      };
    // Fallback to defaults
    const builtin = DEFAULT_CATEGORIES.find((c) => c.id === categoryId);
    if (builtin) return builtin;
    return { id: categoryId, name: categoryId, icon: "📝" };
  };

  return (
    <SettingsGroup
      title={t("settings.ramble.appMappings.title", "Application Mappings")}
    >
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-mid-gray/5 transition-colors"
      >
        <div className="flex items-center gap-2">
          <span className="text-sm text-mid-gray">
            {t(
              "settings.ramble.appMappings.description",
              "Override which prompt type is used for specific applications",
            )}
          </span>
          {mappings.length > 0 && (
            <span className="text-xs bg-logo-primary/20 text-logo-primary px-2 py-0.5 rounded-full">
              {mappings.length}
            </span>
          )}
        </div>
        {isExpanded ? (
          <ChevronDown className="h-4 w-4 text-mid-gray" />
        ) : (
          <ChevronRight className="h-4 w-4 text-mid-gray" />
        )}
      </button>

      {isExpanded && (
        <div className="px-4 pb-4 space-y-4">
          {/* Default Category Selector */}
          <div className="space-y-2">
            <h4 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
              {t(
                "settings.ramble.appMappings.defaultCategory",
                "Default Category",
              )}
            </h4>
            <div className="flex items-center gap-2">
              <span className="text-sm text-mid-gray">
                {t(
                  "settings.ramble.appMappings.defaultCategoryDescription",
                  "Used for apps without a specific mapping:",
                )}
              </span>
              <select
                value={defaultCategoryId}
                onChange={(e) => handleDefaultCategoryChange(e.target.value)}
                className="px-3 py-1.5 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
              >
                {(promptCategories.length > 0
                  ? promptCategories
                  : DEFAULT_CATEGORIES
                ).map((cat) => (
                  <option key={cat.id} value={cat.id}>
                    {cat.icon} {cat.name}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Current mappings */}
          {mappings.length > 0 && (
            <div className="space-y-2">
              <h4 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
                {t("settings.ramble.appMappings.current", "Current Overrides")}
              </h4>
              <div className="space-y-1">
                {mappings.map((mapping) => {
                  const category = getCategoryInfo(mapping.category_id);
                  return (
                    <div
                      key={mapping.bundle_identifier}
                      className="flex items-center justify-between py-2 px-3 bg-mid-gray/5 rounded-lg"
                    >
                      <div className="flex items-center gap-3 flex-1 min-w-0">
                        <span className="text-sm font-medium truncate">
                          {mapping.display_name}
                        </span>
                        <span className="text-xs text-mid-gray truncate">
                          {mapping.bundle_identifier}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <select
                          value={mapping.category_id}
                          onChange={(e) =>
                            handleCategoryChange(
                              mapping.bundle_identifier,
                              mapping.display_name,
                              e.target.value,
                            )
                          }
                          className="text-sm px-2 py-1 bg-background border border-mid-gray/30 rounded focus:outline-none focus:border-logo-primary"
                        >
                          {(promptCategories.length > 0
                            ? promptCategories
                            : DEFAULT_CATEGORIES
                          ).map((cat) => (
                            <option key={cat.id} value={cat.id}>
                              {cat.icon} {cat.name}
                            </option>
                          ))}
                        </select>
                        <button
                          onClick={() =>
                            handleRemoveMapping(mapping.bundle_identifier)
                          }
                          className="p-1 text-mid-gray hover:text-red-500 transition-colors"
                          title={t(
                            "settings.ramble.appMappings.remove",
                            "Remove",
                          )}
                        >
                          <Trash2 className="h-4 w-4" />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Add new mapping */}
          <div className="space-y-2">
            <h4 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
              {t("settings.ramble.appMappings.addNew", "Add Override")}
            </h4>
            <div className="flex items-center gap-2">
              {/* App search/select */}
              <div className="relative flex-1">
                <input
                  type="text"
                  value={selectedApp ? selectedApp.name : searchTerm}
                  onChange={(e) => {
                    setSearchTerm(e.target.value);
                    setSelectedApp(null);
                  }}
                  placeholder={
                    isLoading
                      ? t(
                          "settings.ramble.appMappings.loading",
                          "Loading apps...",
                        )
                      : t(
                          "settings.ramble.appMappings.searchPlaceholder",
                          "Search for an application...",
                        )
                  }
                  disabled={isLoading}
                  className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary disabled:opacity-50"
                />
                {searchTerm && !selectedApp && filteredApps.length > 0 && (
                  <div className="absolute z-10 w-full mt-1 bg-background border border-mid-gray/30 rounded-lg shadow-lg max-h-48 overflow-y-auto">
                    {filteredApps.slice(0, 20).map((app) => (
                      <button
                        key={app.bundleId}
                        onClick={() => {
                          setSelectedApp(app);
                          setSearchTerm("");
                          if (app.suggestedCategory) {
                            setSelectedCategory(app.suggestedCategory);
                          }
                        }}
                        className="w-full px-3 py-2 text-left hover:bg-mid-gray/10 flex items-center justify-between"
                      >
                        <span className="text-sm">{app.name}</span>
                        <span className="text-xs text-mid-gray">
                          {app.source === "detected" && "📍"}
                          {app.source === "known" && "✓"}
                          {app.source === "installed" && "💻"}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {/* Category select */}
              <select
                value={selectedCategory}
                onChange={(e) => setSelectedCategory(e.target.value)}
                className="px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
              >
                {(promptCategories.length > 0
                  ? promptCategories
                  : DEFAULT_CATEGORIES
                ).map((cat) => (
                  <option key={cat.id} value={cat.id}>
                    {cat.icon} {cat.name}
                  </option>
                ))}
              </select>

              {/* Add button */}
              <button
                onClick={handleAddMapping}
                disabled={!selectedApp || isAdding}
                className="flex items-center gap-1 px-3 py-2 bg-logo-primary text-white rounded-lg text-sm hover:bg-logo-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <Plus className="h-4 w-4" />
                {t("settings.ramble.appMappings.add", "Add")}
              </button>
            </div>
            <p className="text-xs text-mid-gray">
              {t(
                "settings.ramble.appMappings.hint",
                "📍 = Previously used, ✓ = Known app, 💻 = Installed",
              )}
            </p>
          </div>
        </div>
      )}
    </SettingsGroup>
  );
};
