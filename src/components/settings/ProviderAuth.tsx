import React from "react";
import { useOAuth, OAuthStatus } from "../../hooks/useOAuth";
import {
  LogIn,
  LogOut,
  RefreshCw,
  AlertCircle,
  CheckCircle,
} from "lucide-react";
import { Button } from "../ui/Button";

interface ProviderAuthProps {
  providerId: string;
  supportsOAuth: boolean;
  authMethod: "api_key" | "oauth";
  apiKey: string;
  onAuthMethodChange: (method: "api_key" | "oauth") => void;
  onApiKeyChange: (apiKey: string) => void;
  /** If true, don't show the auth method toggle (auth method is fixed by the provider) */
  fixedAuthMethod?: boolean;
}

export const ProviderAuth: React.FC<ProviderAuthProps> = ({
  providerId,
  supportsOAuth,
  authMethod,
  apiKey,
  onAuthMethodChange,
  onApiKeyChange,
  fixedAuthMethod = false,
}) => {
  const {
    status,
    isLoading,
    error,
    isAuthenticating,
    startAuth,
    logout,
    refreshToken,
  } = useOAuth(providerId);

  // Format expiration time
  const formatExpiry = (expiresAt: number | null): string => {
    if (!expiresAt) return "";
    const date = new Date(expiresAt * 1000);
    return date.toLocaleString();
  };

  // Check if token is expiring soon (within 1 hour)
  const isExpiringSoon = (expiresAt: number | null): boolean => {
    if (!expiresAt) return false;
    const now = Date.now() / 1000;
    return expiresAt - now < 3600;
  };

  // If provider doesn't support OAuth, or auth method is fixed to API key, show only API key input
  if (!supportsOAuth || (fixedAuthMethod && authMethod === "api_key")) {
    return (
      <div className="space-y-2">
        <label className="text-sm font-medium">API Key</label>
        <input
          type="password"
          value={apiKey}
          onChange={(e) => onApiKeyChange(e.target.value)}
          placeholder="Enter API key..."
          className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
        />
      </div>
    );
  }

  // If auth method is fixed to OAuth, don't show the toggle
  const showAuthMethodToggle = !fixedAuthMethod;

  return (
    <div className="space-y-4">
      {/* Auth Method Toggle - only show if not fixed */}
      {showAuthMethodToggle && (
        <div className="space-y-2">
          <label className="text-sm font-medium">Authentication Method</label>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => onAuthMethodChange("oauth")}
              className={`flex-1 py-2 px-4 text-sm rounded-lg border transition-colors ${
                authMethod === "oauth"
                  ? "bg-logo-primary text-white border-logo-primary"
                  : "bg-mid-gray/5 border-mid-gray/20 hover:border-mid-gray/40"
              }`}
            >
              Sign in with Account
            </button>
            <button
              type="button"
              onClick={() => onAuthMethodChange("api_key")}
              className={`flex-1 py-2 px-4 text-sm rounded-lg border transition-colors ${
                authMethod === "api_key"
                  ? "bg-logo-primary text-white border-logo-primary"
                  : "bg-mid-gray/5 border-mid-gray/20 hover:border-mid-gray/40"
              }`}
            >
              API Key
            </button>
          </div>
        </div>
      )}

      {authMethod === "oauth" ? (
        <div className="space-y-3">
          {/* OAuth Status */}
          {isLoading ? (
            <div className="flex items-center gap-2 text-sm text-mid-gray">
              <RefreshCw className="h-4 w-4 animate-spin" />
              Checking authentication status...
            </div>
          ) : status?.authenticated ? (
            <div className="p-3 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
              <div className="flex items-center gap-2 text-green-700 dark:text-green-400">
                <CheckCircle className="h-4 w-4" />
                <span className="font-medium">Authenticated</span>
              </div>
              {status.email && (
                <p className="text-sm text-green-600 dark:text-green-500 mt-1">
                  {status.email}
                </p>
              )}
              {status.expires_at && (
                <p className="text-xs text-green-600/80 dark:text-green-500/80 mt-1">
                  {isExpiringSoon(status.expires_at) ? (
                    <span className="text-orange-600 dark:text-orange-400">
                      Expires soon: {formatExpiry(status.expires_at)}
                    </span>
                  ) : (
                    <>Expires: {formatExpiry(status.expires_at)}</>
                  )}
                </p>
              )}
            </div>
          ) : (
            <div className="p-3 bg-mid-gray/5 border border-mid-gray/20 rounded-lg">
              <p className="text-sm text-mid-gray">
                Sign in with your account to use this provider without an API
                key.
              </p>
            </div>
          )}

          {/* Error Display */}
          {error && (
            <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
              <div className="flex items-center gap-2 text-red-700 dark:text-red-400">
                <AlertCircle className="h-4 w-4" />
                <span className="text-sm">{error}</span>
              </div>
            </div>
          )}

          {/* OAuth Actions */}
          <div className="flex gap-2">
            {status?.authenticated ? (
              <>
                {isExpiringSoon(status.expires_at) && (
                  <Button
                    onClick={() => refreshToken()}
                    disabled={isAuthenticating}
                    variant="secondary"
                    className="flex items-center gap-2"
                  >
                    <RefreshCw
                      className={`h-4 w-4 ${isAuthenticating ? "animate-spin" : ""}`}
                    />
                    Refresh Token
                  </Button>
                )}
                <Button
                  onClick={() => logout()}
                  variant="secondary"
                  className="flex items-center gap-2 text-red-600 hover:text-red-700"
                >
                  <LogOut className="h-4 w-4" />
                  Sign Out
                </Button>
              </>
            ) : (
              <Button
                onClick={() => startAuth()}
                disabled={isAuthenticating}
                variant="primary"
                className="flex items-center gap-2"
              >
                {isAuthenticating ? (
                  <>
                    <RefreshCw className="h-4 w-4 animate-spin" />
                    Signing in...
                  </>
                ) : (
                  <>
                    <LogIn className="h-4 w-4" />
                    Sign in with {getProviderDisplayName(providerId)}
                  </>
                )}
              </Button>
            )}
          </div>
        </div>
      ) : (
        /* API Key Input */
        <div className="space-y-2">
          <label className="text-sm font-medium">API Key</label>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => onApiKeyChange(e.target.value)}
            placeholder="Enter API key..."
            className="w-full px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm focus:outline-none focus:border-logo-primary"
          />
        </div>
      )}
    </div>
  );
};

/**
 * Get a display-friendly name for a provider
 */
function getProviderDisplayName(providerId: string): string {
  const id = providerId.toLowerCase();
  if (id === "google" || id === "gemini" || id === "gemini_oauth") {
    return "Google";
  }
  if (id === "openai" || id === "openai_oauth") {
    return "OpenAI";
  }
  return providerId;
}

/**
 * Simple OAuth status indicator component
 */
interface OAuthStatusBadgeProps {
  providerId: string;
  supportsOAuth: boolean;
  authMethod: "api_key" | "oauth";
}

export const OAuthStatusBadge: React.FC<OAuthStatusBadgeProps> = ({
  providerId,
  supportsOAuth,
  authMethod,
}) => {
  const { status, isLoading } = useOAuth(providerId);

  if (!supportsOAuth || authMethod !== "oauth") {
    return null;
  }

  if (isLoading) {
    return (
      <span className="text-xs text-mid-gray bg-mid-gray/10 px-2 py-0.5 rounded">
        ...
      </span>
    );
  }

  if (status?.authenticated) {
    return (
      <span className="text-xs text-green-600 bg-green-100 dark:bg-green-900/30 px-2 py-0.5 rounded">
        OAuth
      </span>
    );
  }

  return (
    <span className="text-xs text-orange-600 bg-orange-100 dark:bg-orange-900/30 px-2 py-0.5 rounded">
      Not signed in
    </span>
  );
};
