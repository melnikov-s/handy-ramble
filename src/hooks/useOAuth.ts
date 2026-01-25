import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

// OAuth types (will be auto-generated in bindings.ts after Rust rebuild)
export interface OAuthStatus {
  authenticated: boolean;
  email: string | null;
  expires_at: number | null;
}

interface AuthStartResult {
  auth_url: string;
  state: string;
}

interface AuthResult {
  success: boolean;
  email: string | null;
  error: string | null;
}

interface UseOAuthReturn {
  // State
  status: OAuthStatus | null;
  isLoading: boolean;
  error: string | null;
  isAuthenticating: boolean;

  // Actions
  startAuth: () => Promise<void>;
  logout: () => Promise<void>;
  refreshToken: () => Promise<boolean>;
  refreshStatus: () => Promise<void>;
}

/**
 * Hook for managing OAuth authentication for a specific provider
 *
 * @param providerId - The provider ID (e.g., "google", "openai", "gemini")
 * @returns OAuth state and actions
 */
export const useOAuth = (providerId: string): UseOAuthReturn => {
  const [status, setStatus] = useState<OAuthStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isAuthenticating, setIsAuthenticating] = useState(false);

  // Fetch initial status
  const refreshStatus = useCallback(async () => {
    if (!providerId) {
      setStatus(null);
      setIsLoading(false);
      return;
    }

    try {
      setIsLoading(true);
      setError(null);
      const result = await invoke<OAuthStatus>("oauth_get_status", {
        provider: providerId,
      });
      setStatus(result);
    } catch (err) {
      setError(String(err));
      setStatus(null);
    } finally {
      setIsLoading(false);
    }
  }, [providerId]);

  // Initialize on mount and when provider changes
  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  // Start OAuth flow
  const startAuth = useCallback(async () => {
    if (!providerId) {
      setError("No provider specified");
      return;
    }

    try {
      setIsAuthenticating(true);
      setError(null);

      console.log("[OAuth] Starting auth for provider:", providerId);

      // Start the OAuth flow (opens browser)
      const startResult = await invoke<AuthStartResult>("oauth_start_auth", {
        provider: providerId,
      });

      console.log(
        "[OAuth] Auth started, waiting for callback. State:",
        startResult.state,
      );

      // Wait for callback (this blocks until user completes auth or timeout)
      const authResult = await invoke<AuthResult>("oauth_await_callback", {
        provider: providerId,
        state: startResult.state,
      });

      console.log("[OAuth] Callback received:", authResult);

      if (authResult.success) {
        console.log("[OAuth] Auth successful, refreshing status");
        // Refresh status to get the new state
        await refreshStatus();
      } else {
        console.error("[OAuth] Auth failed:", authResult.error);
        setError(authResult.error || "Authentication failed");
      }
    } catch (err) {
      console.error("[OAuth] Error:", err);
      setError(String(err));
    } finally {
      setIsAuthenticating(false);
    }
  }, [providerId, refreshStatus]);

  // Logout
  const logout = useCallback(async () => {
    if (!providerId) return;

    try {
      setError(null);
      await invoke("oauth_logout", { provider: providerId });
      setStatus({
        authenticated: false,
        email: null,
        expires_at: null,
      });
    } catch (err) {
      setError(String(err));
    }
  }, [providerId]);

  // Refresh token
  const refreshToken = useCallback(async (): Promise<boolean> => {
    if (!providerId) return false;

    try {
      setError(null);
      const success = await invoke<boolean>("oauth_refresh_token", {
        provider: providerId,
      });
      if (success) {
        await refreshStatus();
      }
      return success;
    } catch (err) {
      setError(String(err));
      return false;
    }
  }, [providerId, refreshStatus]);

  return {
    status,
    isLoading,
    error,
    isAuthenticating,
    startAuth,
    logout,
    refreshToken,
    refreshStatus,
  };
};

/**
 * Check if a provider supports OAuth authentication
 */
export const useSupportsOAuth = (providerId: string): boolean => {
  const [supportsOAuth, setSupportsOAuth] = useState(false);

  useEffect(() => {
    if (!providerId) {
      setSupportsOAuth(false);
      return;
    }

    invoke<boolean>("oauth_supports_provider", { providerId })
      .then((result) => setSupportsOAuth(result))
      .catch(() => setSupportsOAuth(false));
  }, [providerId]);

  return supportsOAuth;
};

/**
 * Get the access token for making authenticated API calls
 * Returns null if not authenticated or token expired
 */
export const getOAuthAccessToken = async (
  providerId: string,
): Promise<string | null> => {
  try {
    return await invoke<string | null>("oauth_get_access_token", {
      provider: providerId,
    });
  } catch {
    return null;
  }
};

/**
 * Get request headers for making authenticated API calls
 */
export const getOAuthRequestHeaders = async (
  providerId: string,
): Promise<Record<string, string>> => {
  try {
    return await invoke<Record<string, string>>("oauth_get_request_headers", {
      provider: providerId,
    });
  } catch {
    return {};
  }
};
