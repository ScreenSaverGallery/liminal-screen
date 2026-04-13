// Liminal Screen API Security Module
// Provides shared secret authentication for secure remote options integration

/**
 * Security configuration for Liminal API
 */
export interface SecurityConfig {
  /** Shared secret key for authentication */
  sharedSecret?: string;

  /** Whether to require authentication */
  requireAuth?: boolean;

  /** Session timeout in milliseconds */
  sessionTimeout?: number;
}

/**
 * Authentication token structure
 */
export interface AuthToken {
  /** Timestamp when token was issued */
  timestamp: number;

  /** Random nonce for replay protection */
  nonce: string;

  /** HMAC signature */
  signature: string;
}

/**
 * Security manager for Liminal API
 */
export class SecurityManager {
  private config: Required<SecurityConfig>;
  private activeSessions: Map<string, number> = new Map(); // token -> expiry timestamp
  private cleanupTimer: ReturnType<typeof setInterval> | null = null;

  constructor(config?: SecurityConfig) {
    this.config = {
      sharedSecret:
        config?.sharedSecret ??
        (typeof process !== "undefined" && process.env
          ? process.env.LIMINAL_API_SECRET
          : undefined) ??
        "",
      requireAuth: config?.requireAuth ?? false,
      sessionTimeout: config?.sessionTimeout || 3600000, // 1 hour default
    };

    // Periodic session cleanup
    this.cleanupTimer = setInterval(() => {
      this.cleanupExpiredSessions();
    }, 60000); // Clean up every minute
  }

  /**
   * Destroy the security manager and clean up resources
   */
  destroy(): void {
    if (this.cleanupTimer) {
      clearInterval(this.cleanupTimer);
      this.cleanupTimer = null;
    }
  }

  /**
   * Generate an authentication token
   */
  async generateAuthToken(): Promise<AuthToken | null> {
    if (!this.config.sharedSecret || !this.config.requireAuth) {
      return null;
    }

    const timestamp = Date.now();
    const nonce = this.generateNonce();
    const data = `${timestamp}:${nonce}`;
    const signature = await this.createSignature(data);

    return {
      timestamp,
      nonce,
      signature,
    };
  }

  /**
   * Validate an authentication token
   */
  async validateAuthToken(token: AuthToken): Promise<boolean> {
    if (!this.config.requireAuth) {
      return true; // Authentication not required
    }

    if (!this.config.sharedSecret) {
      console.warn("Authentication required but no shared secret configured");
      return false;
    }

    // Check timestamp (prevent replay attacks)
    const now = Date.now();
    if (now - token.timestamp > this.config.sessionTimeout) {
      console.warn("Authentication token expired");
      return false;
    }

    // Verify signature
    const data = `${token.timestamp}:${token.nonce}`;
    const expectedSignature = await this.createSignature(data);

    if (token.signature !== expectedSignature) {
      console.warn("Invalid authentication signature");
      return false;
    }

    // Check for replay (store used nonces temporarily)
    const tokenKey = `${token.timestamp}:${token.nonce}`;
    if (this.activeSessions.has(tokenKey)) {
      console.warn("Replay attack detected");
      return false;
    }

    // Store session
    this.activeSessions.set(tokenKey, now + this.config.sessionTimeout);
    this.cleanupExpiredSessions();

    return true;
  }

  /**
   * Create HMAC-SHA256 signature for data
   */
  private async createSignature(data: string): Promise<string> {
    if (!this.config.sharedSecret) {
      throw new Error("No shared secret configured");
    }

    const encoder = new TextEncoder();
    const keyData = encoder.encode(this.config.sharedSecret);
    const key = await crypto.subtle.importKey(
      "raw",
      keyData,
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["sign"],
    );
    const signature = await crypto.subtle.sign(
      "HMAC",
      key,
      encoder.encode(data),
    );
    return Array.from(new Uint8Array(signature))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
  }

  /**
   * Generate cryptographically secure random nonce
   */
  private generateNonce(length: number = 16): string {
    const bytes = new Uint8Array(length);
    crypto.getRandomValues(bytes);
    const chars =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let result = "";
    for (let i = 0; i < length; i++) {
      result += chars[bytes[i] % chars.length];
    }
    return result;
  }

  /**
   * Clean up expired sessions
   */
  private cleanupExpiredSessions(): void {
    const now = Date.now();
    for (const [key, expiry] of this.activeSessions.entries()) {
      if (now > expiry) {
        this.activeSessions.delete(key);
      }
    }
  }

  /**
   * Check if authentication is required
   */
  isAuthenticationRequired(): boolean {
    return this.config.requireAuth;
  }

  /**
   * Get security configuration status
   */
  getSecurityStatus(): {
    enabled: boolean;
    sessionTimeout: number;
  } {
    return {
      enabled: this.config.requireAuth,
      sessionTimeout: this.config.sessionTimeout,
    };
  }
}

// Export singleton instance
export const securityManager = new SecurityManager();
